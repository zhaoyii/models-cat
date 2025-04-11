//! 一个简单的文件锁实现，用于防止多个进程同时访问同一个文件

use crate::OpsError;
use std::fs::File;
use std::path::PathBuf;

pub struct FsLock {
    file: std::fs::File,
    path: std::path::PathBuf,
}

impl FsLock {
    pub fn lock(path: PathBuf) -> Result<FsLock, OpsError> {
        let mut path = path.to_path_buf();
        path.set_extension("lock");
        let file = File::create(path.clone())?;
        let mut res = lock(&file);
        for _ in 0..5 {
            if res == 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
            res = lock(&file);
        }
        if res != 0 {
            Err(OpsError::LockAcquisition(path))
        } else {
            Ok(Self { file, path })
        }
    }

    pub fn unlock(&mut self) {
        unlock(&self.file);
        std::fs::remove_file(&self.path).unwrap();
    }
}

#[cfg(target_family = "unix")]
mod unix {
    use std::os::fd::AsRawFd;

    pub(crate) fn lock(file: &std::fs::File) -> i32 {
        unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) }
    }
    pub(crate) fn unlock(file: &std::fs::File) -> i32 {
        unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_UN) }
    }
}
#[cfg(target_family = "unix")]
use unix::{lock, unlock};

#[cfg(target_family = "windows")]
mod windows {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Storage::FileSystem::{
        LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY, LockFileEx, UnlockFile,
    };

    pub(crate) fn lock(file: &std::fs::File) -> i32 {
        unsafe {
            let mut overlapped = std::mem::zeroed();
            let flags = LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY;
            let res = LockFileEx(
                file.as_raw_handle() as HANDLE,
                flags,
                0,
                !0,
                !0,
                &mut overlapped,
            );
            1 - res
        }
    }
    pub(crate) fn unlock(file: &std::fs::File) -> i32 {
        unsafe { UnlockFile(file.as_raw_handle() as HANDLE, 0, 0, !0, !0) }
    }
}
#[cfg(target_family = "windows")]
use windows::{lock, unlock};

#[cfg(not(any(target_family = "unix", target_family = "windows")))]
mod other {
    pub(crate) fn lock(file: &std::fs::File) -> i32 {
        unimplemented!("not supported on this platform")
    }
    pub(crate) fn unlock(file: &std::fs::File) -> i32 {
        unimplemented!("not supported on this platform")
    }
}
#[cfg(not(any(target_family = "unix", target_family = "windows")))]
use other::{lock, unlock};

#[cfg(test)]
mod tests {
    #[test]
    fn test_lock_unlock() {
        let mut lock = crate::fslock::FsLock::lock(std::path::PathBuf::from("test.lock")).unwrap();
        lock.unlock();
    }
}
