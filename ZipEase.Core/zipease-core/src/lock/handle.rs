#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LockHandle(u64);

impl LockHandle {
    pub const INVALID: isize = -1;

    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn as_raw(&self) -> isize {
        self.0 as isize
    }

    pub fn from_raw(raw: isize) -> Option<Self> {
        if raw <= 0 { None } else { Some(Self(raw as u64)) }
    }

    pub fn id(&self) -> u64 {
        self.0
    }
}
