use std::collections::HashMap;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;

use crate::*;

const MAX_HEAP_SIZE: usize = 1 * 1000 * 1000 * 1000; // 1 GB

struct Heap {
    allocations: HashMap<usize, Allocation>,
    total_bytes: Arc<AtomicUsize>,
}

struct Allocation {
    buffer: Vec<u8>,
    flags: usize,
    child: Vec<Allocation>,
    heap: Arc<AtomicUsize>,
}

impl Allocation {
    fn new(buffer: Vec<u8>, flags: usize, heap: Arc<AtomicUsize>) -> Self {
        let child = Vec::with_capacity(0);
        Self { buffer, flags, child, heap }
    }
}

impl Drop for Allocation {
    fn drop(&mut self) {
        self.heap.fetch_sub(self.buffer.len(), Ordering::SeqCst);
    }
}

unsafe impl Send for Heap {}

impl Heap {
    fn new() -> Self {
        let allocations = HashMap::new();
        let total_bytes = Arc::new(AtomicUsize::new(0));
        Heap { allocations, total_bytes }
    }

    fn allocate_buffer(&mut self, size: usize, flags: ULONG, data: *mut LPVOID) -> HRESULT {
        if data.is_null() {
            xfs_reject!(WFS_ERR_INVALID_POINTER);
        }
        let mut allocation = match self.try_allocate(size, flags as usize) {
            Ok(allocation) => allocation,
            Err(error) => return error,
        };
        let pointer = allocation.buffer.as_mut_ptr() as LPVOID;

        // SAFETY: the pointer is not null
        unsafe { data.write(pointer) };

        self.allocations.insert(pointer as usize, allocation);
        WFS_SUCCESS
    }

    fn allocate_more(&mut self, size: usize, original: LPVOID, data: *mut LPVOID) -> HRESULT {
        if data.is_null() || original.is_null() {
            xfs_reject!(WFS_ERR_INVALID_POINTER);
        }
        if unsafe { *data } == original {
            xfs_reject!(WFS_ERR_INVALID_POINTER);
        }
        let flags = match self.allocations.get(&(original as usize)) {
            Some(allocation) => allocation.flags,
            None => xfs_reject!(WFS_ERR_INVALID_BUFFER),
        };
        let mut allocation = match self.try_allocate(size, flags) {
            Ok(allocation) => allocation,
            Err(error) => xfs_reject!(error),
        };
        let pointer = allocation.buffer.as_mut_ptr() as LPVOID;

        // SAFETY: the pointer is not null
        unsafe { data.write(pointer) };

        self.allocations.get_mut(&(original as usize)).unwrap().child.push(allocation);
        WFS_SUCCESS
    }

    fn deallocate(&mut self, data: LPVOID) -> HRESULT {
        if data.is_null() {
            xfs_reject!(WFS_ERR_INVALID_POINTER);
        }
        if self.allocations.remove(&(data as usize)).is_none() {
            xfs_reject!(WFS_ERR_INVALID_BUFFER);
        }
        WFS_SUCCESS
    }

    fn try_allocate(&mut self, size: usize, flags: usize) -> Result<Allocation, HRESULT> {
        let new_size = self.total_bytes.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
            value.checked_add(size as usize).and_then(|new| if new > MAX_HEAP_SIZE { None } else { Some(new) })
        });
        if new_size.is_err() {
            xfs_reject_err!(WFS_ERR_OUT_OF_MEMORY);
        }
        let buffer = vec![0; size as usize];
        let allocation = Allocation::new(buffer, flags, self.total_bytes.clone());
        Ok(allocation)
    }
}

lazy_static::lazy_static! {
    static ref HEAP: Mutex<Heap> = Mutex::new(Heap::new());
}

fn get_heap<'a>() -> MutexGuard<'a, Heap> {
    HEAP.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn allocate_buffer(size: ULONG, flags: ULONG, data: *mut LPVOID) -> HRESULT {
    get_heap().allocate_buffer(size as usize, flags, data)
}

pub fn allocate_more(size: ULONG, original: LPVOID, data: *mut LPVOID) -> HRESULT {
    get_heap().allocate_more(size as usize, original, data)
}

pub fn free_buffer(data: LPVOID) -> HRESULT {
    get_heap().deallocate(data)
}

// tests
#[cfg(test)]
mod tests {
    use crate::heap::*;
    use std::ptr;

    #[test]
    fn test_allocate() {
        let mut heap = Heap::new();
        assert_eq!(heap.allocate_buffer(10, 0, &mut ptr::null_mut()), WFS_SUCCESS);
        assert_eq!(heap.allocate_buffer(100, 0, &mut ptr::null_mut()), WFS_SUCCESS);
        assert_eq!(heap.allocate_buffer(1000, 0, &mut ptr::null_mut()), WFS_SUCCESS);
        assert_eq!(heap.allocate_buffer(10000, 0, &mut ptr::null_mut()), WFS_SUCCESS);
        assert_eq!(heap.allocate_buffer(100000, 0, &mut ptr::null_mut()), WFS_SUCCESS);
        assert_eq!(heap.allocate_buffer(1000000, 0, &mut ptr::null_mut()), WFS_SUCCESS);
    }

    #[test]
    fn test_allocate_more() {
        let mut heap = Heap::new();
        let mut ptr = ptr::null_mut();
        assert_eq!(heap.allocate_buffer(10, 0, &mut ptr), WFS_SUCCESS);
        assert_eq!(heap.allocate_more(100, ptr, &mut ptr::null_mut()), WFS_SUCCESS);
        assert_eq!(heap.allocate_more(1000, ptr, &mut ptr::null_mut()), WFS_SUCCESS);
        assert_eq!(heap.allocate_more(10000, ptr, &mut ptr::null_mut()), WFS_SUCCESS);
        assert_eq!(heap.allocate_more(100000, ptr, &mut ptr::null_mut()), WFS_SUCCESS);
        assert_eq!(heap.allocate_more(1000000, ptr, &mut ptr::null_mut()), WFS_SUCCESS);
    }

    #[test]
    fn test_free() {
        let mut heap = Heap::new();
        let mut ptr = ptr::null_mut();
        assert_eq!(heap.allocate_buffer(10, 0, &mut ptr), WFS_SUCCESS);
        assert_eq!(heap.deallocate(ptr), WFS_SUCCESS);
        assert_eq!(heap.deallocate(ptr), WFS_ERR_INVALID_BUFFER);

        assert_eq!(heap.allocate_buffer(100, 0, &mut ptr), WFS_SUCCESS);
        assert_eq!(heap.deallocate(ptr), WFS_SUCCESS);
        assert_eq!(heap.deallocate(ptr), WFS_ERR_INVALID_BUFFER);

        assert_eq!(heap.allocate_buffer(1000, 0, &mut ptr), WFS_SUCCESS);
        assert_eq!(heap.deallocate(ptr), WFS_SUCCESS);
        assert_eq!(heap.deallocate(ptr), WFS_ERR_INVALID_BUFFER);

        assert_eq!(heap.allocate_buffer(10000, 0, &mut ptr), WFS_SUCCESS);
        assert_eq!(heap.deallocate(ptr), WFS_SUCCESS);
        assert_eq!(heap.deallocate(ptr), WFS_ERR_INVALID_BUFFER);

        assert_eq!(heap.allocate_buffer(100000, 0, &mut ptr), WFS_SUCCESS);
        assert_eq!(heap.deallocate(ptr), WFS_SUCCESS);
        assert_eq!(heap.deallocate(ptr), WFS_ERR_INVALID_BUFFER);

        assert_eq!(heap.allocate_buffer(1000000, 0, &mut ptr), WFS_SUCCESS);
        assert_eq!(heap.deallocate(ptr), WFS_SUCCESS);
        assert_eq!(heap.deallocate(ptr), WFS_ERR_INVALID_BUFFER);
    }

    #[test]
    fn free_child_first() {
        let mut heap = Heap::new();
        let mut parent = ptr::null_mut();
        let mut child = ptr::null_mut();
        assert_eq!(heap.allocate_buffer(10, 0, &mut parent), WFS_SUCCESS);
        assert_eq!(heap.allocate_more(10, parent, &mut child), WFS_SUCCESS);
        assert_eq!(heap.deallocate(child), WFS_ERR_INVALID_BUFFER);
        assert_eq!(heap.deallocate(parent), WFS_SUCCESS);
        assert_eq!(heap.deallocate(child), WFS_ERR_INVALID_BUFFER);
    }

    #[test]
    fn free_parent_first() {
        let mut heap = Heap::new();
        let mut parent = ptr::null_mut();
        let mut child = ptr::null_mut();
        assert_eq!(heap.allocate_buffer(10, 0, &mut parent), WFS_SUCCESS);
        assert_eq!(heap.allocate_more(10, parent, &mut child), WFS_SUCCESS);
        assert_eq!(heap.deallocate(parent), WFS_SUCCESS);
        assert_eq!(heap.deallocate(parent), WFS_ERR_INVALID_BUFFER);
        assert_eq!(heap.deallocate(child), WFS_ERR_INVALID_BUFFER);
    }

    #[test]
    fn test_allocate_invalid_ptr() {
        let mut heap = Heap::new();
        assert_eq!(heap.allocate_buffer(20, WFS_MEM_ZEROINIT, ptr::null_mut()), WFS_ERR_INVALID_POINTER);
    }

    #[test]
    fn test_allocate_oom() {
        let mut heap = Heap::new();
        assert_eq!(heap.allocate_buffer(MAX_HEAP_SIZE + 1, WFS_MEM_ZEROINIT, &mut ptr::null_mut()), WFS_ERR_OUT_OF_MEMORY);
    }

    #[test]
    fn test_allocate_more_oom() {
        let mut heap = Heap::new();
        let mut data = ptr::null_mut();
        assert_eq!(heap.allocate_buffer(MAX_HEAP_SIZE / 2, WFS_MEM_ZEROINIT, &mut data), WFS_SUCCESS);
        assert_eq!(heap.allocate_more(MAX_HEAP_SIZE, data, &mut ptr::null_mut()), WFS_ERR_OUT_OF_MEMORY);
    }

    #[test]
    fn test_allocate_more_invalid_ptr() {
        let mut heap = Heap::new();
        let mut data = ptr::null_mut();
        assert_eq!(heap.allocate_buffer(20, 0, &mut data), WFS_SUCCESS);
        assert_eq!(heap.allocate_more(20, data, &mut data), WFS_ERR_INVALID_POINTER);
        assert_eq!(heap.allocate_more(10, ptr::null_mut(), ptr::null_mut()), WFS_ERR_INVALID_POINTER);
        assert_eq!(heap.allocate_more(10, 1 as *mut _, &mut ptr::null_mut()), WFS_ERR_INVALID_BUFFER);
    }
}
