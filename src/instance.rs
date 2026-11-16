//! One Grammata per user owns the session. Launching it again hands the paths
//! to the running one and exits; `--new-window` starts an independent guest
//! that neither reads nor writes the session, so two windows never overwrite
//! each other's unsaved text.
//!
//! On Unix the owner holds an exclusive lock on a file (the kernel drops it if
//! the process dies) and listens on a socket next to it. Elsewhere there is no
//! single instance: every process owns the session, as before.

use std::path::PathBuf;
use std::sync::mpsc::Receiver;

/// What this process is, once started.
pub struct Instance {
    pub owns_session: bool,
    /// Paths sent by later launches (owner only).
    pub inbox: Option<Receiver<Vec<PathBuf>>>,
    #[cfg(unix)]
    _lock: Option<std::fs::File>,
}

impl Instance {
    /// A window that does not own the session (`--new-window`, or no owner reachable).
    pub fn guest() -> Self {
        Self {
            owns_session: false,
            inbox: None,
            #[cfg(unix)]
            _lock: None,
        }
    }

    /// The owner of the session, with nobody to listen to (tests, Windows).
    pub fn solo() -> Self {
        Self {
            owns_session: true,
            inbox: None,
            #[cfg(unix)]
            _lock: None,
        }
    }
}

pub enum Claim {
    /// Another instance took the paths; this process should exit.
    Forwarded,
    Owner(Owner),
    Guest,
}

pub struct Owner {
    #[cfg(unix)]
    lock: Option<std::fs::File>,
    #[cfg(unix)]
    listener: Option<std::os::unix::net::UnixListener>,
}

#[cfg(unix)]
mod imp {
    use super::*;
    use std::ffi::OsStr;
    use std::io::{Read, Write};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::DirBuilderExt;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::Path;
    use std::time::Duration;

    /// Per-user directory for the lock and the socket (mode 0700).
    pub fn runtime_dir() -> PathBuf {
        match std::env::var_os("XDG_RUNTIME_DIR") {
            Some(d) if !d.is_empty() => PathBuf::from(d).join("grammata"),
            _ => {
                let user = std::env::var("USER").unwrap_or_else(|_| "user".into());
                std::env::temp_dir().join(format!("grammata-{user}"))
            }
        }
    }

    pub fn claim_in(dir: &Path, paths: &[PathBuf]) -> Claim {
        let made = std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir);
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(dir.join("instance.lock"));
        let (Ok(()), Ok(lock)) = (made, lock) else {
            // No place for the lock: run as before, owning the session.
            return Claim::Owner(Owner { lock: None, listener: None });
        };
        let sock = dir.join("instance.sock");
        match lock.try_lock() {
            Ok(()) => {
                // Only the lock holder may touch the socket: any file left is stale.
                let _ = std::fs::remove_file(&sock);
                let listener = UnixListener::bind(&sock).ok();
                Claim::Owner(Owner { lock: Some(lock), listener })
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                if forward(&sock, paths) {
                    Claim::Forwarded
                } else {
                    Claim::Guest
                }
            }
            Err(_) => Claim::Owner(Owner { lock: None, listener: None }),
        }
    }

    /// Sends the paths to the owner and waits for its acknowledgement. The owner
    /// may still be binding the socket, so connecting is retried for ~3 s.
    fn forward(sock: &Path, paths: &[PathBuf]) -> bool {
        let mut stream = None;
        for _ in 0..30 {
            if let Ok(s) = UnixStream::connect(sock) {
                stream = Some(s);
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let Some(mut s) = stream else { return false };
        let _ = s.set_read_timeout(Some(Duration::from_secs(3)));
        let mut msg = Vec::new();
        for p in paths {
            msg.extend_from_slice(p.as_os_str().as_bytes());
            msg.push(0);
        }
        let mut ack = Vec::new();
        s.write_all(&msg).is_ok()
            && s.shutdown(std::net::Shutdown::Write).is_ok()
            && s.read_to_end(&mut ack).is_ok()
            && ack == b"ok"
    }

    pub fn parse(msg: &[u8]) -> Vec<PathBuf> {
        msg.split(|&b| b == 0)
            .filter(|p| !p.is_empty())
            .map(|p| PathBuf::from(OsStr::from_bytes(p)))
            .collect()
    }

    impl Owner {
        /// Starts listening for later launches. `wake` runs after each message
        /// (to repaint the window).
        pub fn serve(self, wake: impl Fn() + Send + 'static) -> Instance {
            let Some(listener) = self.listener else {
                return Instance { owns_session: true, inbox: None, _lock: self.lock };
            };
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::Builder::new()
                .name("instance-server".into())
                .spawn(move || {
                    for mut conn in listener.incoming().flatten() {
                        let _ = conn.set_read_timeout(Some(Duration::from_secs(2)));
                        let mut msg = Vec::new();
                        // Paths only: a megabyte is far more than any command line.
                        if (&mut conn).take(1 << 20).read_to_end(&mut msg).is_err() {
                            continue;
                        }
                        let paths = parse(&msg);
                        if tx.send(paths).is_err() {
                            break;
                        }
                        let _ = conn.write_all(b"ok");
                        wake();
                    }
                })
                .ok();
            Instance { owns_session: true, inbox: Some(rx), _lock: self.lock }
        }
    }
}

#[cfg(unix)]
pub fn claim(paths: &[PathBuf]) -> Claim {
    imp::claim_in(&imp::runtime_dir(), paths)
}

#[cfg(not(unix))]
pub fn claim(_paths: &[PathBuf]) -> Claim {
    Claim::Owner(Owner {})
}

#[cfg(not(unix))]
impl Owner {
    pub fn serve(self, _wake: impl Fn() + Send + 'static) -> Instance {
        Instance::solo()
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::imp::*;
    use super::*;
    use std::time::Duration;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("grammata-inst-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn the_first_owns_and_the_second_forwards_its_paths() {
        let d = dir("fwd");
        let Claim::Owner(owner) = claim_in(&d, &[]) else {
            panic!("the first launch must own the session");
        };
        let inst = owner.serve(|| {});
        assert!(inst.owns_session);
        let sent = vec![PathBuf::from("/tmp/a.typ"), PathBuf::from("/tmp/b b.typ")];
        assert!(matches!(claim_in(&d, &sent), Claim::Forwarded));
        let got = inst
            .inbox
            .as_ref()
            .unwrap()
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        assert_eq!(got, sent);
        // Launching with no paths still reaches the owner (to bring it to the front).
        assert!(matches!(claim_in(&d, &[]), Claim::Forwarded));
        assert!(
            inst.inbox
                .as_ref()
                .unwrap()
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_stale_socket_does_not_block_a_new_owner() {
        let d = dir("stale");
        let Claim::Owner(owner) = claim_in(&d, &[]) else {
            panic!("owner");
        };
        let inst = owner.serve(|| {});
        // The process "dies": the lock is released, the socket file stays.
        drop(inst);
        assert!(d.join("instance.sock").exists());
        assert!(matches!(claim_in(&d, &[]), Claim::Owner(_)));
    }

    #[test]
    fn paths_with_spaces_and_unicode_survive() {
        let p = PathBuf::from("/tmp/mi carpeta/ñandú.typ");
        let mut msg = std::os::unix::ffi::OsStrExt::as_bytes(p.as_os_str()).to_vec();
        msg.push(0);
        assert_eq!(parse(&msg), vec![p]);
    }
}
