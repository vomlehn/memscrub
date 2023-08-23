extern crate libc;

use libc::{c_void, MAP_FAILED, MAP_SHARED, PROT_READ, PROT_WRITE, MAP_PRIVATE, MAP_ANONYMOUS, madvise, MADV_RANDOM};
use std::fs::File;
use std::io::{BufRead, Read, Seek, SeekFrom};
use std::os::unix::io::AsRawFd;
use std::ptr;
use std::process::{Command, Stdio};

use libmemscrub::{CacheDesc, Cacheline, MemoryScrubber, ScrubArea};

type ECCData = u64;
const MY_CACHELINE_ITEMS: usize = 8;
const MY_CACHE_INDEX_WIDTH: usize = 10;

#[repr(C)]
struct MyCacheline {
    data: [ECCData; MY_CACHELINE_ITEMS],
}

impl Cacheline for MyCacheline {}

#[derive(Clone)]
struct MyCacheDesc {
    cache_index_width: usize,
}

impl CacheDesc<MyCacheline> for MyCacheDesc {
    fn cache_index_width(&self) -> usize {
        self.cache_index_width
    }

    fn read_cacheline(&mut self, cacheline_ptr: *const MyCacheline) {
        let cacheline = unsafe { &*cacheline_ptr };
        let cacheline_data = &cacheline.data[0];
        let _dummy = unsafe { ptr::read(cacheline_data) };
    }
}

static MY_CACHE_DESC: MyCacheDesc = MyCacheDesc {
    cache_index_width: MY_CACHE_INDEX_WIDTH,
};

fn main() -> std::io::Result<()> {
    let mut my_cache_desc = MY_CACHE_DESC.clone();
    let mut my_scrub_areas = read_scrub_areas();

    let mut total: usize = 0;
    for scrub_area in &my_scrub_areas {
        let delta = my_cache_desc.size_in_cachelines(&scrub_area);
        total += delta;
    }
    total *= my_cache_desc.cacheline_size();
    println!("total size {}", total);

    // Open the file for reading
    let mut file = File::open("/dev/mem")?;

    for scrub_area in &mut my_scrub_areas {
        // Define the start and end offsets
        let mut start_offset = scrub_area.start as usize; // Specify your start offset here
        let mut end_offset = scrub_area.end as usize;   // Specify your end offset here

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
println!("mmap failed");
            return Err(std::io::Error::last_os_error());
        }

        start_offset -= data as usize;
        end_offset -= data as usize;
        scrub_area.start = start_offset as *const u8;
        scrub_area.end = end_offset as *const u8;
    }

    let scrubber = MemoryScrubber::<MyCacheDesc,
        MyCacheline>::new(&mut my_cache_desc, &my_scrub_areas);
    match scrubber {
        Err(e) => panic!("Failed to create memory scrubber: {}", e),
        Ok(_) => {},
    };
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
    let mut my_scrub_areas: Vec<ScrubArea> = Vec::new();

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
                my_scrub_areas.push(scrub_area);
            }
        }
    }

    // Print the tuples in the vector
    for scrub_area in &my_scrub_areas {
        println!("{:?}", scrub_area);
    }

    my_scrub_areas
}

/*
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::io::AsRawFd;
use std::ptr;
use libc::{c_void, MAP_FAILED, MAP_SHARED, PROT_READ, PROT_WRITE, MAP_PRIVATE, MAP_ANONYMOUS, madvise, MADV_RANDOM};

fn main() -> std::io::Result<()> {
    // Open the file for reading
    let mut file = File::open("path/to/your/file")?;

    // Define the start and end offsets
    let start_offset = 100; // Specify your start offset here
    let end_offset = 300;   // Specify your end offset here

    // Calculate the length of the mapped portion
    let length = end_offset - start_offset;

    // Seek to the start offset in the file
    file.seek(SeekFrom::Start(start_offset as u64))?;

    // Allocate memory to map the file portion
    let mut data: *mut c_void = ptr::null_mut();
    unsafe {
        data = libc::mmap(
            ptr::null_mut(),
            length as usize,
            PROT_READ | PROT_WRITE, // Read and write access
            MAP_SHARED,             // Share with other processes
            file.as_raw_fd(),
            start_offset as i64,
        );
    }

    if data == MAP_FAILED {
        return Err(std::io::Error::last_os_error());
    }

    // Do something with the mapped data
    // ...

    // Unmap the memory when done
    unsafe {
        libc::munmap(data, length as usize);
    }

    Ok(())
}
*/
