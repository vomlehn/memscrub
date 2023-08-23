extern crate libc;

use libc::{c_void, MAP_FAILED, MAP_SHARED, PROT_READ};
use std::fs::File;
use std::io::{BufRead, Seek, SeekFrom};
use std::os::unix::io::AsRawFd;
use std::process::{Command, Stdio};
use std::ptr;

use libmemscrub_arch::{BaseCacheDesc, CACHE_DESC, CacheDesc,
    Cacheline, MemoryScrubber, ScrubArea};

fn main() -> std::io::Result<()> {
    let mut cache_desc = CACHE_DESC.clone();
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
        println!("{:?}", scrub_area);
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
println!("mmap failed");
            return Err(std::io::Error::last_os_error());
        }

        let end = data as usize + length - 1;
        let scrub_area =
            ScrubArea { start: data as *const u8, end: end as *const u8, };
        virt_scrub_areas.push(scrub_area);
    }

    // Print the tuples in the vector
    println!("Mapped addresses:");
    for scrub_area in &virt_scrub_areas {
        println!("{:?}", scrub_area);
    }

    let mut scrubber = match MemoryScrubber::<CacheDesc,
        Cacheline>::new(&mut cache_desc, &virt_scrub_areas) {
        Err(e) => panic!("Failed to create memory scrubber: {}", e),
        Ok(scrubber) => scrubber,
    };

    match scrubber.scrub(total_bytes) {
        Err(e) => panic!("Scrub failed: {}", e),
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
