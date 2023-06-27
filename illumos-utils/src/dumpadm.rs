use std::io::SeekFrom;
use camino::Utf8PathBuf;
use std::process::Command;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

#[derive(thiserror::Error, Debug)]
pub enum DumpHdrError {
    #[error("I/O error while attempting to read dumphdr: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Invalid magic number {0} (expected 0xdefec8ed)")]
    InvalidMagic(u32),

    #[error("Invalid dumphdr version {0} (expected 10)")]
    InvalidVersion(u32),
}

pub async fn dump_flag_is_valid(dump_slice: &Utf8PathBuf) -> Result<bool, DumpHdrError> {
    // values from /usr/src/uts/common/sys/dumphdr.h:
    const DUMP_OFFSET: u64 = 65536; // pad at start/end of dev

    const DUMP_MAGIC: u32 = 0xdefec8ed; // weird hex but ok
    const DUMP_VERSION: u32 = 10; // version of this dumphdr

    const DF_VALID: u32 = 0x00000001; // Dump is valid (savecore clears)

    let raw_path = format!("{},raw", dump_slice);

    let mut f = File::open(raw_path).await?;
    f.seek(SeekFrom::Start(DUMP_OFFSET)).await?;

    // read the first few fields of dumphdr.
    // typedef struct dumphdr {
    //     uint32_t dump_magic;
    //     uint32_t dump_version;
    //     uint32_t dump_flags;
    //     /* [...] */
    // }

    let magic = f.read_u32().await?;
    if magic != DUMP_MAGIC {
        return Err(DumpHdrError::InvalidMagic(magic));
    }

    let version = f.read_u32().await?;
    if version != DUMP_VERSION {
        return Err(DumpHdrError::InvalidVersion(version));
    }

    let flags = f.read_u32().await?;
    Ok((flags & DF_VALID) != 0)
}

const DUMPADM: &str = "/usr/sbin/dumpadm";

#[derive(thiserror::Error, Debug)]
pub enum DumpAdmError {
    #[error("Error obtaining or modifying dump configuration. dump_slice: {dump_slice}, savecore_dir: {savecore_dir:?}")]
    Execution {
        dump_slice: Utf8PathBuf,
        savecore_dir: Option<Utf8PathBuf>,
    },

    #[error("Invalid invocation of dumpadm: {0:?}")]
    InvalidCommand(Vec<String>),

    #[error("dumpadm process was terminated by a signal.")]
    TerminatedBySignal,

    #[error("dumpadm invocation exited with unexpected return code {0}")]
    UnexpectedExitCode(i32),

    #[error("Failed to create placeholder savecore directory at /tmp/crash: {0}")]
    Mkdir(std::io::Error),

    #[error("Failed to execute dumpadm process: {0}")]
    Exec(std::io::Error),
}

pub fn dumpadm(dump_slice: &Utf8PathBuf, savecore_dir: Option<&Utf8PathBuf>) -> Result<(), DumpAdmError> {
    let mut cmd = Command::new(DUMPADM);
    cmd.env_clear();

    // Include memory from the current process if there is one for the panic
    // context, in addition to kernel memory:
    cmd.arg("-c").arg("curproc");

    // Use the given block device path for dump storage:
    cmd.arg("-d").arg(&dump_slice);

    // Compress crash dumps:
    cmd.arg("-z").arg("on");

    if let Some(savecore_dir) = savecore_dir {
        // Run savecore(8) to place the existing contents of dump_slice (if
        // any) into savecore_dir, and clear the presence flag.
        cmd.arg("-s").arg(savecore_dir);
    } else {
        // Do not run savecore(8) automatically...
        cmd.arg("-n");

        // ...but do create and use a tmpfs path (rather than the default
        // location under /var/crash, which is in the ramdisk pool), because
        // dumpadm refuses to do what we ask otherwise.
        let tmp_crash = "/tmp/crash";
        std::fs::create_dir_all(tmp_crash).map_err(DumpAdmError::Mkdir)?;

        cmd.arg("-s").arg(tmp_crash);
    }

    let out = cmd.output().map_err(DumpAdmError::Exec)?;

    match out.status.code() {
        Some(0) => Ok(()),
        Some(1) => Err(DumpAdmError::Execution {
            dump_slice: dump_slice.clone(),
            savecore_dir: savecore_dir.cloned(),
        }),
        Some(2) => {
            // unwrap: every arg we've provided in this function is UTF-8
            let mut args = vec![cmd.get_program().to_str().unwrap().to_string()];
            cmd.get_args().for_each(|arg| args.push(arg.to_str().unwrap().to_string()));
            Err(DumpAdmError::InvalidCommand(args))
        }
        Some(n) => Err(DumpAdmError::UnexpectedExitCode(n)),
        None => Err(DumpAdmError::TerminatedBySignal),
    }
}
