//! A machine-wide queue for video renders: concurrent `plan render` runs, from
//! any worktree, wait for a free slot instead of contending for the same cores
//! and GPU. Frames, snapshots, and `verify` do not queue. Slots are advisory
//! file locks in the temporary directory, released when the holder exits.
use std::{
    env,
    fs::{self, File, OpenOptions, TryLockError},
    io::Write,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use anyhow::{Context, Result};

/// How many renders may run at once; `0` turns the queue off.
const SLOTS: &str = "PSYCHOPOMP_RENDER_SLOTS";
const DEFAULT_SLOTS: usize = 1;
const POLL: Duration = Duration::from_millis(500);

/// A held render slot, released on drop.
pub(super) struct Slot {
    _lock: Option<File>,
}

/// Wait for a render slot, saying who holds them while waiting.
pub(super) fn wait() -> Result<Slot> {
    let slots = slots(env::var(SLOTS).ok().as_deref())?;
    if slots == 0 {
        return Ok(Slot { _lock: None });
    }
    let dir = env::temp_dir();
    let mut waiting = false;
    loop {
        if let Some(file) = try_take(&dir, slots)? {
            if waiting {
                eprintln!("render slot acquired");
            }
            return Ok(Slot { _lock: Some(file) });
        }
        if !waiting {
            eprintln!(
                "waiting for a render slot held by {}; {SLOTS}=0 skips the queue",
                holders(&dir, slots)
            );
            waiting = true;
        }
        thread::sleep(POLL);
    }
}

fn slots(value: Option<&str>) -> Result<usize> {
    match value.map(str::trim) {
        None | Some("") => Ok(DEFAULT_SLOTS),
        Some(value) => value
            .parse()
            .with_context(|| format!("{SLOTS}={value}: expected a number of renders")),
    }
}

fn slot_path(dir: &Path, index: usize) -> PathBuf {
    dir.join(format!("psychopomp-render-{index}.lock"))
}

/// The first free slot of `slots`, locked and stamped with this process ID.
fn try_take(dir: &Path, slots: usize) -> Result<Option<File>> {
    for index in 0..slots {
        let path = slot_path(dir, index);
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .with_context(|| format!("open render slot {}", path.display()))?;
        match file.try_lock() {
            Ok(()) => {
                file.set_len(0)?;
                write!(file, "{}", std::process::id())?;
                return Ok(Some(file));
            }
            Err(TryLockError::WouldBlock) => continue,
            Err(TryLockError::Error(error)) => {
                return Err(error).with_context(|| format!("lock {}", path.display()));
            }
        }
    }
    Ok(None)
}

fn holders(dir: &Path, slots: usize) -> String {
    let pids = (0..slots)
        .filter_map(|index| fs::read_to_string(slot_path(dir, index)).ok())
        .map(|pid| pid.trim().to_owned())
        .filter(|pid| !pid.is_empty())
        .collect::<Vec<_>>();
    if pids.is_empty() {
        "another render".to_owned()
    } else {
        format!("PID {}", pids.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{holders, slots, try_take};

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "psychopomp-render-queue-{name}-{}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn slot_counts_default_to_one_and_zero_disables() {
        assert_eq!(slots(None).unwrap(), 1);
        assert_eq!(slots(Some(" ")).unwrap(), 1);
        assert_eq!(slots(Some("0")).unwrap(), 0);
        assert_eq!(slots(Some("3")).unwrap(), 3);
        assert!(slots(Some("many")).is_err());
    }

    #[test]
    fn renders_beyond_the_slot_count_wait_until_one_is_released() {
        let dir = scratch("slots");
        let first = try_take(&dir, 2).unwrap().expect("first slot");
        let second = try_take(&dir, 2).unwrap().expect("second slot");
        assert!(try_take(&dir, 2).unwrap().is_none(), "both slots are held");
        assert!(holders(&dir, 2).contains(&std::process::id().to_string()));
        drop(first);
        let third = try_take(&dir, 2).unwrap();
        assert!(third.is_some(), "a released slot is free again");
        drop((second, third));
        fs::remove_dir_all(&dir).unwrap();
    }
}
