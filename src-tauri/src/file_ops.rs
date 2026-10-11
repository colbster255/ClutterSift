//! Windows file moves bound to an open object, never a shell command or delete fallback.
use crate::policy;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::os::windows::{ffi::OsStrExt, fs::OpenOptionsExt, io::AsRawHandle};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};
use windows_sys::Win32::{Foundation::HANDLE, Storage::FileSystem::*};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    volume: u32,
    index: u64,
    bytes: u64,
    written: u64,
    hash: String,
}

pub struct LockedFile {
    file: File,
    // Every directory from drive root down is pinned against write/delete opens.
    _parents: Vec<File>,
}
fn open(path: &Path, directory: bool) -> Result<File, String> {
    std::fs::OpenOptions::new()
        .access_mode(if directory {
            FILE_READ_ATTRIBUTES
        } else {
            FILE_READ_DATA | FILE_READ_ATTRIBUTES | 0x10000
        })
        .share_mode(FILE_SHARE_READ)
        .custom_flags(
            FILE_FLAG_OPEN_REPARSE_POINT
                | if directory {
                    FILE_FLAG_BACKUP_SEMANTICS
                } else {
                    0
                },
        )
        .open(path)
        .map_err(|e| format!("Cannot lock {}: {e}", path.display()))
}
fn info(file: &File) -> Result<BY_HANDLE_FILE_INFORMATION, String> {
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: the File owns a live handle; info points to initialized writable storage.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle() as HANDLE, &mut info) } == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err("Reparse points are never moved or restored.".into());
    }
    Ok(info)
}
pub fn pin_directory(path: &Path) -> Result<Vec<File>, String> {
    let checked = policy::checked_path(path)?;
    if policy::normalized(path.to_str().ok_or("Unsupported non-Unicode path.")?)
        != policy::normalized(checked.to_str().ok_or("Unsupported non-Unicode path.")?)
    {
        return Err(format!(
            "Directory resolution changed the requested location: {} → {}",
            path.display(),
            checked.display()
        ));
    }
    let path = checked;
    let mut parents = path.ancestors().collect::<Vec<_>>();
    parents.reverse();
    let mut handles = Vec::new();
    for parent in parents {
        let handle = open(parent, true)?;
        let metadata = info(&handle)?;
        if metadata.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
            return Err("Expected an ordinary directory.".into());
        }
        handles.push(handle);
    }
    Ok(handles)
}
impl LockedFile {
    pub fn open(path: &Path) -> Result<Self, String> {
        let checked = policy::checked_path(path)?;
        if policy::normalized(path.to_str().ok_or("Unsupported non-Unicode path.")?)
            != policy::normalized(checked.to_str().ok_or("Unsupported non-Unicode path.")?)
        {
            return Err("File resolution changed the reviewed location.".into());
        }
        let path = checked;
        let parents = pin_directory(path.parent().ok_or("Missing parent directory.")?)?;
        let file = open(&path, false)?;
        let metadata = info(&file)?;
        if metadata.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0 || metadata.nNumberOfLinks != 1
        {
            return Err("Only ordinary files with a single hard link are supported.".into());
        }
        // Resolve the opened object, rather than trusting a previously resolved pathname.
        let mut name = vec![0u16; 32768];
        // SAFETY: owned live file handle and correctly sized UTF-16 output buffer.
        let length = unsafe {
            GetFinalPathNameByHandleW(
                file.as_raw_handle() as HANDLE,
                name.as_mut_ptr(),
                name.len() as u32,
                0,
            )
        } as usize;
        if length == 0 || length >= name.len() {
            return Err("Could not resolve the opened file.".into());
        }
        let actual =
            String::from_utf16(&name[..length]).map_err(|_| "Unsupported non-Unicode path.")?;
        let expected = path.to_str().ok_or("Unsupported non-Unicode path.")?;
        if policy::normalized(&actual) != policy::normalized(expected) {
            return Err("The opened object is not at the reviewed path.".into());
        }
        Ok(Self {
            file,
            _parents: parents,
        })
    }
    pub fn identity(&mut self) -> Result<Identity, String> {
        self.identity_cancellable(&std::sync::atomic::AtomicBool::new(false))
    }
    pub fn identity_cancellable(
        &mut self,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<Identity, String> {
        let meta = info(&self.file)?;
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|e| e.to_string())?;
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 65536];
        loop {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err("File verification cancelled.".into());
            }
            let count = self.file.read(&mut buffer).map_err(|e| e.to_string())?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
        Ok(Identity {
            volume: meta.dwVolumeSerialNumber,
            index: ((meta.nFileIndexHigh as u64) << 32) | meta.nFileIndexLow as u64,
            bytes: ((meta.nFileSizeHigh as u64) << 32) | meta.nFileSizeLow as u64,
            written: ((meta.ftLastWriteTime.dwHighDateTime as u64) << 32)
                | meta.ftLastWriteTime.dwLowDateTime as u64,
            hash: format!("{:x}", hash.finalize()),
        })
    }
    pub fn move_to(&self, destination: &Path) -> Result<(), String> {
        let _destination_guards =
            pin_directory(destination.parent().ok_or("Missing destination parent.")?)?;
        let name = destination.as_os_str().encode_wide().collect::<Vec<_>>();
        if name.contains(&0) || name.len() > 32760 {
            return Err("Unsupported destination path.".into());
        }
        let offset = std::mem::offset_of!(FILE_RENAME_INFO, FileName);
        // The Win32 FileName field is NUL-terminated even though FileNameLength excludes it.
        let length = offset + (name.len() + 1) * 2;
        // usize storage provides alignment for the native structure and flexible UTF-16 tail.
        let mut storage = vec![0usize; length.div_ceil(std::mem::size_of::<usize>())];
        let rename = storage.as_mut_ptr().cast::<FILE_RENAME_INFO>();
        // SAFETY: aligned zeroed allocation covers the structure header and complete tail;
        // source handle stays live; Windows copies the buffer synchronously. Flags=0 forbids replacement.
        let result = unsafe {
            (*rename).Anonymous.Flags = 0;
            (*rename).RootDirectory = std::ptr::null_mut();
            (*rename).FileNameLength = (name.len() * 2) as u32;
            std::ptr::copy_nonoverlapping(
                name.as_ptr(),
                (*rename).FileName.as_mut_ptr(),
                name.len(),
            );
            SetFileInformationByHandle(
                self.file.as_raw_handle() as HANDLE,
                FileRenameInfo,
                rename.cast(),
                length as u32,
            )
        };
        if result == 0 {
            return Err(format!(
                "Recoverable move failed; no delete fallback: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rename_refuses_occupied_names_and_preserves_variable_length_names() {
        let dir = tempfile::tempdir().unwrap();
        for length in 1..33 {
            let source = dir.path().join(format!("source-{length}.tmp"));
            let occupied = dir
                .path()
                .join(format!("{}-occupied.tmp", "a".repeat(length)));
            let fresh = dir.path().join(format!("{}-資料.held", "b".repeat(length)));
            std::fs::write(&source, b"source bytes").unwrap();
            std::fs::write(&occupied, b"keep existing bytes").unwrap();
            let locked = LockedFile::open(&source).unwrap();
            assert!(
                locked.move_to(&occupied).is_err(),
                "occupied name length {length}"
            );
            assert_eq!(std::fs::read(&occupied).unwrap(), b"keep existing bytes");
            assert!(source.exists());
            locked.move_to(&fresh).unwrap();
            drop(locked);
            assert!(!source.exists());
            assert_eq!(std::fs::read(&fresh).unwrap(), b"source bytes");
        }
    }
    #[test]
    fn handle_move_does_not_replace_or_follow_hard_links() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.tmp");
        let b = dir.path().join("b.tmp");
        std::fs::write(&a, "first").unwrap();
        std::fs::write(&b, "second").unwrap();
        let locked = LockedFile::open(&a).unwrap();
        assert!(locked.move_to(&b).is_err());
        assert_eq!(std::fs::read(&b).unwrap(), b"second");
        assert!(std::fs::write(&a, "mutate").is_err());
        assert!(std::fs::rename(&a, dir.path().join("swapped.tmp")).is_err());
        drop(locked);
        let link = dir.path().join("hard.tmp");
        std::fs::hard_link(&a, &link).unwrap();
        assert!(LockedFile::open(&a).is_err());
    }
    #[test]
    fn directory_chain_is_pinned_and_unicode_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("資料");
        std::fs::create_dir(&folder).unwrap();
        let a = folder.join("🧹.tmp");
        std::fs::write(&a, "bytes").unwrap();
        let mut locked = LockedFile::open(&a).unwrap();
        let identity = locked.identity().unwrap();
        assert!(std::fs::rename(&folder, dir.path().join("other")).is_err());
        let b = folder.join("保留.held");
        locked.move_to(&b).unwrap();
        drop(locked);
        assert_eq!(LockedFile::open(&b).unwrap().identity().unwrap(), identity);
    }
}
