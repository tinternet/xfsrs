use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::ops::{Deref, DerefMut};
use std::sync::Mutex;

use lazy_static::lazy_static;
use winapi::shared::minwindef::LPVOID;

pub struct Data {
    ptr: LPVOID,
}

impl Data {
    pub fn new(ptr: LPVOID) -> Self {
        Self { ptr }
    }
}

impl PartialEq for Data {
    fn eq(&self, other: &Self) -> bool {
        self.ptr == other.ptr
    }
}

impl Hash for Data {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ptr.hash(state);
    }
}

impl Eq for Data {}
unsafe impl Send for Data {}

pub struct List {
    list: VecDeque<LPVOID>,
}

impl List {
    pub fn new() -> Self {
        Self { list: VecDeque::new() }
    }
}

impl Deref for List {
    type Target = VecDeque<LPVOID>;

    fn deref(&self) -> &Self::Target {
        &self.list
    }
}

impl DerefMut for List {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.list
    }
}

unsafe impl Send for List {}

lazy_static! {
    pub static ref ALLOC_MAP: Mutex<HashMap<Data, List>> = Mutex::new(HashMap::new());
}
