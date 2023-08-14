use std::ptr;

use libmemscrub::{CacheDesc, MemoryScrubber};

const N_CACHELINE_ECCDATA: usize = 8;
type MyCacheline = [ECCData; N_CACHELINE_ECCDATA];

type ECCData = u64;

const ECC_DATA: MyCacheline = [0; N_CACHELINE_ECCDATA];

struct MyCacheDesc {
}

impl CacheDesc<MyCacheline> for MyCacheDesc {
    fn cache_index_width(&self) -> usize {
        10
    }

    fn read_cacheline(&self, cacheline: *const MyCacheline) {
        let _dummy = unsafe {
            ptr::read(&((*cacheline)[0]) as *const _)
        };
    }
}

fn main() {
}

mod tests {
    use super::*;

    #[test]
    fn test_main() {
        const MEM_AREA_CACHELINES: usize = 12;

        let mem: Vec<MyCacheline> = vec![ECC_DATA; MEM_AREA_CACHELINES];
        let size = mem.len() * std::mem::size_of_val(&mem[0]);
        let ptr = mem.as_ptr() as *const u8;
        let cache_desc = MyCacheDesc {};

        let mut scrubber = match MemoryScrubber::<MyCacheline>::
            new(&cache_desc, ptr, size) {
            Err(e) => panic!("Could not create MemoryScrubber: {}",
                e),
            Ok(scrubber) => scrubber,
        };

        match scrubber.scrub(size / 4) {
            Err(e) => panic!("Scrub failed: {}", e),
            Ok(_) => {},
        }
    }
}
