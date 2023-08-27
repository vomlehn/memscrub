extern crate libc;

use libc::{c_void, MAP_FAILED, MAP_SHARED, PROT_READ};
use std::fs::File;
use std::io::{BufRead, Seek, SeekFrom};
use std::os::unix::io::AsRawFd;
use std::process::{Command, Stdio};
use std::ptr;

use memscrublib_base::{CacheDesc, Cacheline, MemoryScrubber, ScrubArea};

// Basic configuration
const CACHE_INDEX_WIDTH: usize = 10;
const CACHELINE_ITEMS: usize = 8;

type MyECCData = u64;

struct MyCacheline {
    data: [MyECCData; CACHELINE_ITEMS],
}

impl Cacheline for MyCacheline {
}

struct MyCacheDesc {
    my_cache_index_width: usize,
}

impl MyCacheDesc {
    fn new(my_cache_index_width: usize) -> MyCacheDesc {
        MyCacheDesc {
            my_cache_index_width: my_cache_index_width,
        }
    }
}

impl CacheDesc<MyCacheline> for MyCacheDesc {
    fn cache_index_width(&self) -> usize {
        self.my_cache_index_width
    }

    fn read_cacheline(&mut self, cacheline_ptr: *const MyCacheline) {
        // Get a safe reference to the cache line
        let cacheline = unsafe { &*cacheline_ptr };
        // Get a reference to the first element
        let cacheline_data = &cacheline.data[0];
        // Read from the first element
        let _dummy = unsafe { ptr::read(cacheline_data) };
    }
}

fn main() -> std::io::Result<()> {
    scrub_dev_mem()
}

fn scrub_dev_mem() -> std::io::Result<()> {
    let mut cache_desc = MyCacheDesc::new(CACHE_INDEX_WIDTH);
    let phys_scrub_areas = read_scrub_areas();

    let mut total_bytes: usize = 0;
    for scrub_area in &phys_scrub_areas {
        let delta = cache_desc.size_in_cachelines(&scrub_area);
        total_bytes += delta;
    }
    total_bytes *= cache_desc.cacheline_size();

    // Print the tuples in the vector
    println!("Physical addresses:");
    for scrub_area in &phys_scrub_areas {
        println!("{:p}-{:p}: {}", scrub_area.start, scrub_area.end,
            cache_desc.size_in_cachelines(&scrub_area) <<
            cache_desc.cacheline_width());
    }
    println!("total size {}", total_bytes);

    // Open the file for reading
    let mut file = File::open("/dev/mem")?;
    let mut virt_scrub_areas: Vec<ScrubArea> = Vec::new();

    for scrub_area in &phys_scrub_areas {
        // Define the start and end offsets
        let start_offset = scrub_area.start as usize; // Specify your start offset here
        let end_offset = scrub_area.end as usize;   // Specify your end offset here

        // Calculate the length of the mapped portion
        let length = end_offset - start_offset + 1;

        // Seek to the start offset in the file
        file.seek(SeekFrom::Start(start_offset as u64))?;

        // Allocate memory to map the file portion
        //let mut data: *mut c_void = ptr::null_mut();
        let data: *mut c_void;
        unsafe {
            data = libc::mmap(
                ptr::null_mut(),
                length as usize,
                PROT_READ, // Read and write access
                MAP_SHARED,             // Share with other processes
                file.as_raw_fd(),
                start_offset as i64,
            );
        }

        if data == MAP_FAILED {
            return Err(std::io::Error::last_os_error());
        }

        let end = data as usize + length - 1;
        let virt_scrub_area =
            ScrubArea { start: data as *const u8, end: end as *const u8, };
        virt_scrub_areas.push(virt_scrub_area);
    }

    // Print the tuples in the vector
    println!("Mapped addresses:");
    for scrub_area in &virt_scrub_areas {
        println!("{:p}-{:p}: %{}", scrub_area.start, scrub_area.end,
            cache_desc.size_in_cachelines(&scrub_area) <<
            cache_desc.cacheline_width());
    }

    let mut scrubber = match MemoryScrubber::<MyCacheDesc, MyCacheline>
        ::new(&mut cache_desc, &virt_scrub_areas) {
        Err(_) => panic!("Failed to create memory scrubber"),
        Ok(scrubber) => scrubber,
    };

    match scrubber.scrub(total_bytes) {
        Err(_) => panic!("Scrub failed"),
        Ok(_) => {},
    }

    Ok(())
}

fn read_scrub_areas () -> Vec<ScrubArea> {
    // Command to run an external program
    let output = Command::new("./extract-memconfig")
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start external program")
        .stdout
        .expect("Failed to capture standard output");

    let reader = std::io::BufReader::new(output);

    // Vector to store tuples of usize values
    let mut scrub_areas: Vec<ScrubArea> = Vec::new();

    // Read lines from the program's output and process them
    for line in reader.lines() {
        if let Ok(line) = line {
            let hex_values: Vec<&str> = line
                .trim()
                .split_whitespace()
                .collect();

            if hex_values.len() == 2 {
                // Remove "0x" prefix and convert to usize
                let val1 = usize::from_str_radix(hex_values[0]
                    .trim_start_matches("0x"), 16).unwrap_or(0);
                let val2 = usize::from_str_radix(hex_values[1]
                    .trim_start_matches("0x"), 16).unwrap_or(0);

                let scrub_area = ScrubArea {
                    start: val1 as *const u8, end: val2 as *const u8,
                };
                scrub_areas.push(scrub_area);
            }
        }
    }

    scrub_areas
}
