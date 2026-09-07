//! The catalogue: which Python symbols mean which real-world side effect.
//!
//! Everything here is a static table keyed by *canonical* dotted path — the
//! path after import aliases have been unwound, so `np.load` and
//! `numpy.load` both arrive as `numpy.load`. Method calls on values we have
//! inferred a kind for (`f = open(p)` … `f.write(x)`) arrive as
//! `<file>.write`, which keeps them in the same namespace as everything else.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::model::{Effect, Severity, ValueKind};

/// Where in the call's arguments to look for the thing being acted upon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetArg {
    None,
    /// Positional index.
    Pos(usize),
    /// Keyword name, falling back to a positional index.
    Named(&'static str, usize),
}

#[derive(Clone, Copy, Debug)]
pub struct Rule {
    pub path: &'static str,
    pub effect: Effect,
    pub sev: Severity,
    pub verb: &'static str,
    pub target: TargetArg,
    pub note: Option<&'static str>,
}

const fn r(
    path: &'static str,
    effect: Effect,
    sev: Severity,
    verb: &'static str,
    target: TargetArg,
) -> Rule {
    Rule {
        path,
        effect,
        sev,
        verb,
        target,
        note: None,
    }
}

const fn rn(
    path: &'static str,
    effect: Effect,
    sev: Severity,
    verb: &'static str,
    target: TargetArg,
    note: &'static str,
) -> Rule {
    Rule {
        path,
        effect,
        sev,
        verb,
        target,
        note: Some(note),
    }
}

use Effect::*;
use Severity::{Caution, Info, Notable};
use TargetArg::{Named, None as NoArg, Pos};

#[rustfmt::skip]
pub const RULES: &[Rule] = &[
    // ---- filesystem: read -------------------------------------------------
    r("os.listdir",              FsRead, Info,    "list",   Pos(0)),
    r("os.scandir",              FsRead, Info,    "list",   Pos(0)),
    r("os.walk",                 FsRead, Info,    "walk",   Pos(0)),
    r("os.stat",                 FsRead, Info,    "stat",   Pos(0)),
    r("os.path.exists",          FsRead, Info,    "test",   Pos(0)),
    r("os.path.isfile",          FsRead, Info,    "test",   Pos(0)),
    r("os.path.isdir",           FsRead, Info,    "test",   Pos(0)),
    r("os.path.getsize",         FsRead, Info,    "stat",   Pos(0)),
    r("glob.glob",               FsRead, Info,    "glob",   Pos(0)),
    r("glob.iglob",              FsRead, Info,    "glob",   Pos(0)),
    r("json.load",               FsRead, Info,    "parse",  Pos(0)),
    r("tomllib.load",            FsRead, Info,    "parse",  Pos(0)),
    r("csv.reader",              FsRead, Info,    "parse",  Pos(0)),
    r("csv.DictReader",          FsRead, Info,    "parse",  Pos(0)),
    r("yaml.safe_load",          FsRead, Info,    "parse",  Pos(0)),
    r("pandas.read_csv",         FsRead, Info,    "read",   Named("filepath_or_buffer", 0)),
    r("pandas.read_parquet",     FsRead, Info,    "read",   Pos(0)),
    r("pandas.read_json",        FsRead, Info,    "read",   Pos(0)),
    r("pandas.read_excel",       FsRead, Info,    "read",   Pos(0)),
    r("pandas.read_hdf",         FsRead, Info,    "read",   Pos(0)),
    r("pandas.read_table",       FsRead, Info,    "read",   Pos(0)),
    r("pandas.read_sql",         FsRead, Info,    "query",  Pos(0)),
    r("numpy.load",              FsRead, Info,    "read",   Pos(0)),
    r("numpy.loadtxt",           FsRead, Info,    "read",   Pos(0)),
    r("numpy.genfromtxt",        FsRead, Info,    "read",   Pos(0)),
    r("numpy.fromfile",          FsRead, Info,    "read",   Pos(0)),
    r("cv2.imread",              FsRead, Info,    "read",   Pos(0)),
    r("PIL.Image.open",          FsRead, Info,    "read",   Pos(0)),
    r("nibabel.load",            FsRead, Info,    "read",   Pos(0)),
    r("h5py.File",               FsRead, Info,    "open",   Pos(0)),
    r("<path>.read_text",        FsRead, Info,    "read",   NoArg),
    r("<path>.read_bytes",       FsRead, Info,    "read",   NoArg),
    r("<path>.iterdir",          FsRead, Info,    "list",   NoArg),
    r("<path>.glob",             FsRead, Info,    "glob",   Pos(0)),
    r("<path>.rglob",            FsRead, Info,    "glob",   Pos(0)),
    r("<path>.exists",           FsRead, Info,    "test",   NoArg),
    r("<path>.stat",             FsRead, Info,    "stat",   NoArg),
    r("<file>.read",             FsRead, Info,    "read",   NoArg),
    r("<file>.readlines",        FsRead, Info,    "read",   NoArg),
    r("<file>.readline",         FsRead, Info,    "read",   NoArg),

    // ---- filesystem: write ------------------------------------------------
    r("os.makedirs",             FsWrite, Info,    "mkdir",  Pos(0)),
    r("os.mkdir",                FsWrite, Info,    "mkdir",  Pos(0)),
    r("os.rename",               FsWrite, Notable, "rename", Pos(0)),
    r("os.replace",              FsWrite, Notable, "replace", Pos(1)),
    r("os.symlink",              FsWrite, Notable, "link",   Pos(1)),
    r("os.link",                 FsWrite, Notable, "link",   Pos(1)),
    r("os.chmod",                FsWrite, Notable, "chmod",  Pos(0)),
    r("os.chown",                FsWrite, Notable, "chown",  Pos(0)),
    r("os.truncate",             FsWrite, Notable, "truncate", Pos(0)),
    r("shutil.copy",             FsWrite, Notable, "copy",   Pos(1)),
    r("shutil.copy2",            FsWrite, Notable, "copy",   Pos(1)),
    r("shutil.copyfile",         FsWrite, Notable, "copy",   Pos(1)),
    r("shutil.copytree",         FsWrite, Notable, "copy",   Pos(1)),
    r("shutil.move",             FsWrite, Notable, "move",   Pos(1)),
    r("shutil.make_archive",     FsWrite, Notable, "archive", Pos(0)),
    r("shutil.unpack_archive",   FsWrite, Notable, "unpack", Pos(1)),
    r("json.dump",               FsWrite, Notable, "write",  Pos(1)),
    r("yaml.dump",               FsWrite, Notable, "write",  Pos(1)),
    r("pickle.dump",             FsWrite, Notable, "write",  Pos(1)),
    r("numpy.save",              FsWrite, Notable, "write",  Pos(0)),
    r("numpy.savez",             FsWrite, Notable, "write",  Pos(0)),
    r("numpy.savez_compressed",  FsWrite, Notable, "write",  Pos(0)),
    r("numpy.savetxt",           FsWrite, Notable, "write",  Pos(0)),
    r("torch.save",              FsWrite, Notable, "write",  Pos(1)),
    r("matplotlib.pyplot.savefig", FsWrite, Notable, "plot", Pos(0)),
    r("cv2.imwrite",             FsWrite, Notable, "write",  Pos(0)),
    r("tempfile.NamedTemporaryFile", FsWrite, Info, "tmpfile", NoArg),
    r("tempfile.mkstemp",        FsWrite, Info,    "tmpfile", NoArg),
    r("tempfile.mkdtemp",        FsWrite, Info,    "tmpdir",  NoArg),
    r("tempfile.TemporaryDirectory", FsWrite, Info, "tmpdir", NoArg),
    r("<path>.write_text",       FsWrite, Notable, "write",  NoArg),
    r("<path>.write_bytes",      FsWrite, Notable, "write",  NoArg),
    r("<path>.mkdir",            FsWrite, Info,    "mkdir",  NoArg),
    r("<path>.touch",            FsWrite, Info,    "touch",  NoArg),
    r("<path>.rename",           FsWrite, Notable, "rename", Pos(0)),
    r("<file>.write",            FsWrite, Notable, "write",  NoArg),
    r("<file>.writelines",       FsWrite, Notable, "write",  NoArg),
    r("<frame>.to_csv",          FsWrite, Notable, "write",  Pos(0)),
    r("<frame>.to_parquet",      FsWrite, Notable, "write",  Pos(0)),
    r("<frame>.to_json",         FsWrite, Notable, "write",  Pos(0)),
    r("<frame>.to_excel",        FsWrite, Notable, "write",  Pos(0)),
    r("<frame>.to_pickle",       FsWrite, Notable, "write",  Pos(0)),
    r("<frame>.to_sql",          FsWrite, Notable, "write",  Pos(0)),

    // ---- filesystem: destroy ----------------------------------------------
    rn("os.remove",       FsDelete, Caution, "delete", Pos(0), "removes a file from disk"),
    rn("os.unlink",       FsDelete, Caution, "delete", Pos(0), "removes a file from disk"),
    rn("os.rmdir",        FsDelete, Caution, "rmdir",  Pos(0), "removes a directory"),
    rn("os.removedirs",   FsDelete, Caution, "rmdir",  Pos(0), "removes a directory tree"),
    rn("shutil.rmtree",   FsDelete, Caution, "rmtree", Pos(0), "recursively deletes a directory"),
    rn("<path>.unlink",   FsDelete, Caution, "delete", NoArg,  "removes a file from disk"),
    rn("<path>.rmdir",    FsDelete, Caution, "rmdir",  NoArg,  "removes a directory"),

    // ---- network ----------------------------------------------------------
    r("requests.get",            Net, Notable, "GET",    Pos(0)),
    r("requests.head",           Net, Info,    "HEAD",   Pos(0)),
    rn("requests.post",          Net, Caution, "POST",   Pos(0), "sends data to a remote host"),
    rn("requests.put",           Net, Caution, "PUT",    Pos(0), "sends data to a remote host"),
    rn("requests.patch",         Net, Caution, "PATCH",  Pos(0), "sends data to a remote host"),
    rn("requests.delete",        Net, Caution, "DELETE", Pos(0), "deletes a remote resource"),
    r("requests.request",        Net, Notable, "request", Pos(1)),
    r("requests.Session",        Net, Notable, "session", NoArg),
    r("httpx.get",               Net, Notable, "GET",    Pos(0)),
    rn("httpx.post",             Net, Caution, "POST",   Pos(0), "sends data to a remote host"),
    r("httpx.Client",            Net, Notable, "session", NoArg),
    r("httpx.AsyncClient",       Net, Notable, "session", NoArg),
    r("aiohttp.ClientSession",   Net, Notable, "session", NoArg),
    r("urllib.request.urlopen",  Net, Notable, "fetch",  Pos(0)),
    rn("urllib.request.urlretrieve", Net, Caution, "download", Pos(0), "downloads a remote file to disk"),
    r("socket.socket",           Net, Notable, "socket", NoArg),
    r("socket.create_connection", Net, Notable, "connect", Pos(0)),
    r("http.client.HTTPConnection", Net, Notable, "connect", Pos(0)),
    r("http.client.HTTPSConnection", Net, Notable, "connect", Pos(0)),
    r("ftplib.FTP",              Net, Notable, "connect", Pos(0)),
    r("smtplib.SMTP",            Net, Notable, "mail",   Pos(0)),
    rn("paramiko.SSHClient",     Net, Caution, "ssh",    NoArg, "opens an SSH session"),
    r("boto3.client",            Net, Notable, "aws",    Pos(0)),
    r("boto3.resource",          Net, Notable, "aws",    Pos(0)),
    r("websockets.connect",      Net, Notable, "ws",     Pos(0)),
    r("urllib3.PoolManager",     Net, Notable, "session", NoArg),
    r("<session>.get",           Net, Notable, "GET",    Pos(0)),
    rn("<session>.post",         Net, Caution, "POST",   Pos(0), "sends data to a remote host"),
    r("<session>.request",       Net, Notable, "request", Pos(1)),

    // ---- subprocess / shell ------------------------------------------------
    rn("subprocess.run",         Process, Caution, "run",   Pos(0), "runs an external command"),
    rn("subprocess.Popen",       Process, Caution, "spawn", Pos(0), "spawns an external process"),
    rn("subprocess.call",        Process, Caution, "run",   Pos(0), "runs an external command"),
    rn("subprocess.check_call",  Process, Caution, "run",   Pos(0), "runs an external command"),
    rn("subprocess.check_output", Process, Caution, "run",  Pos(0), "runs an external command"),
    rn("subprocess.getoutput",   Process, Caution, "shell", Pos(0), "runs a command through the shell"),
    rn("subprocess.getstatusoutput", Process, Caution, "shell", Pos(0), "runs a command through the shell"),
    rn("os.system",              Process, Caution, "shell", Pos(0), "runs a command through the shell"),
    rn("os.popen",               Process, Caution, "shell", Pos(0), "runs a command through the shell"),
    rn("os.execv",               Process, Caution, "exec",  Pos(0), "replaces this process"),
    rn("os.execvp",              Process, Caution, "exec",  Pos(0), "replaces this process"),
    rn("os.fork",                Process, Caution, "fork",  NoArg,  "forks the process"),
    rn("pty.spawn",              Process, Caution, "spawn", Pos(0), "spawns an interactive process"),

    // ---- environment / credentials ----------------------------------------
    r("os.getenv",               Env, Info,    "getenv", Pos(0)),
    r("os.putenv",               Env, Notable, "setenv", Pos(0)),
    r("os.environ.get",          Env, Info,    "getenv", Pos(0)),
    r("os.environ.setdefault",   Env, Notable, "setenv", Pos(0)),
    r("dotenv.load_dotenv",      Env, Notable, "dotenv", Pos(0)),
    rn("getpass.getpass",        Env, Notable, "prompt", Pos(0), "reads a secret from the terminal"),
    r("platform.system",         Env, Info,    "probe",  NoArg),
    r("os.uname",                Env, Info,    "probe",  NoArg),
    r("os.getcwd",               Env, Info,    "probe",  NoArg),
    r("os.chdir",                Env, Notable, "chdir",  Pos(0)),

    // ---- dynamic code ------------------------------------------------------
    rn("eval",                   Dynamic, Caution, "eval",   Pos(0), "evaluates code built at runtime"),
    rn("exec",                   Dynamic, Caution, "exec",   Pos(0), "executes code built at runtime"),
    rn("compile",                Dynamic, Caution, "compile", Pos(0), "compiles code built at runtime"),
    rn("__import__",             Dynamic, Caution, "import", Pos(0), "imports a module chosen at runtime"),
    r("importlib.import_module", Dynamic, Notable, "import", Pos(0)),
    rn("pickle.loads",           Dynamic, Caution, "unpickle", NoArg, "unpickling runs arbitrary code"),
    rn("pickle.load",            Dynamic, Caution, "unpickle", Pos(0), "unpickling runs arbitrary code"),
    rn("dill.loads",             Dynamic, Caution, "unpickle", NoArg, "unpickling runs arbitrary code"),
    rn("marshal.loads",          Dynamic, Caution, "unmarshal", NoArg, "decodes raw code objects"),
    rn("yaml.load",              Dynamic, Caution, "yaml",   Pos(0), "yaml.load without SafeLoader can construct objects"),
    rn("torch.load",             Dynamic, Caution, "load",   Pos(0), "torch.load unpickles by default"),
    rn("ctypes.CDLL",            Dynamic, Caution, "dlopen", Pos(0), "loads native code"),
    rn("ctypes.cdll.LoadLibrary", Dynamic, Caution, "dlopen", Pos(0), "loads native code"),

    // ---- output -----------------------------------------------------------
    r("print",                   Stdout, Info, "print", Pos(0)),
    r("pprint.pprint",           Stdout, Info, "print", Pos(0)),
    r("rich.print",              Stdout, Info, "print", Pos(0)),
    r("sys.stdout.write",        Stdout, Info, "print", Pos(0)),
    r("sys.stderr.write",        Stdout, Info, "warn",  Pos(0)),
    r("logging.debug",           Stdout, Info, "log",   Pos(0)),
    r("logging.info",            Stdout, Info, "log",   Pos(0)),
    r("logging.warning",         Stdout, Info, "log",   Pos(0)),
    r("logging.error",           Stdout, Info, "log",   Pos(0)),
    r("logging.exception",       Stdout, Info, "log",   Pos(0)),
    r("tqdm.tqdm",               Stdout, Info, "progress", NoArg),
    r("matplotlib.pyplot.show",  Stdout, Info, "plot",  NoArg),

    // ---- randomness --------------------------------------------------------
    r("random.random",           Random, Info, "random", NoArg),
    r("random.randint",          Random, Info, "random", NoArg),
    r("random.choice",           Random, Info, "random", NoArg),
    r("random.shuffle",          Random, Info, "shuffle", NoArg),
    r("random.sample",           Random, Info, "sample", NoArg),
    r("random.seed",             Random, Info, "seed",   Pos(0)),
    r("numpy.random.rand",       Random, Info, "random", NoArg),
    r("numpy.random.randn",      Random, Info, "random", NoArg),
    r("numpy.random.randint",    Random, Info, "random", NoArg),
    r("numpy.random.seed",       Random, Info, "seed",   Pos(0)),
    r("numpy.random.default_rng", Random, Info, "rng",   Pos(0)),
    r("secrets.token_hex",       Random, Info, "secret", NoArg),
    r("secrets.token_urlsafe",   Random, Info, "secret", NoArg),
    r("uuid.uuid4",              Random, Info, "uuid",   NoArg),
    r("os.urandom",              Random, Info, "random", NoArg),
    r("torch.manual_seed",       Random, Info, "seed",   Pos(0)),

    // ---- time --------------------------------------------------------------
    rn("time.sleep",             Clock, Notable, "sleep", Pos(0), "blocks for a while"),
    r("time.time",               Clock, Info, "now",    NoArg),
    r("time.perf_counter",       Clock, Info, "clock",  NoArg),
    r("time.monotonic",          Clock, Info, "clock",  NoArg),
    r("datetime.datetime.now",   Clock, Info, "now",    NoArg),
    r("datetime.date.today",     Clock, Info, "today",  NoArg),
    r("asyncio.sleep",           Clock, Info, "sleep",  Pos(0)),

    // ---- concurrency -------------------------------------------------------
    r("threading.Thread",        Concurrency, Notable, "thread", NoArg),
    r("multiprocessing.Pool",    Concurrency, Notable, "pool",   Pos(0)),
    r("multiprocessing.Process", Concurrency, Notable, "process", NoArg),
    r("concurrent.futures.ThreadPoolExecutor", Concurrency, Notable, "pool", NoArg),
    r("concurrent.futures.ProcessPoolExecutor", Concurrency, Notable, "pool", NoArg),
    r("asyncio.run",             Concurrency, Info, "async",  NoArg),
    r("asyncio.gather",          Concurrency, Info, "gather", NoArg),
    r("asyncio.create_task",     Concurrency, Info, "task",   NoArg),
    r("joblib.Parallel",         Concurrency, Notable, "parallel", NoArg),

    // ---- termination -------------------------------------------------------
    rn("sys.exit",               Exit, Notable, "exit", Pos(0), "ends the process"),
    rn("os._exit",               Exit, Caution, "exit", Pos(0), "ends the process immediately, skipping cleanup"),
    rn("exit",                   Exit, Notable, "exit", Pos(0), "ends the process"),
    rn("quit",                   Exit, Notable, "exit", Pos(0), "ends the process"),
];

/// Methods distinctive enough to name their effect even when we could not
/// work out what they were called on. `stats.to_csv(path)` writes a file
/// whether or not we managed to prove `stats` is a DataFrame; missing that is
/// worse than the occasional false positive on a same-named method.
#[rustfmt::skip]
pub const METHOD_RULES: &[Rule] = &[
    r("to_csv",      FsWrite, Notable, "write",  Pos(0)),
    r("to_parquet",  FsWrite, Notable, "write",  Pos(0)),
    r("to_json",     FsWrite, Notable, "write",  Pos(0)),
    r("to_excel",    FsWrite, Notable, "write",  Pos(0)),
    r("to_pickle",   FsWrite, Notable, "write",  Pos(0)),
    r("to_sql",      FsWrite, Notable, "write",  Pos(0)),
    r("to_hdf",      FsWrite, Notable, "write",  Pos(0)),
    r("savefig",     FsWrite, Notable, "plot",   Pos(0)),
    r("write_text",  FsWrite, Notable, "write",  NoArg),
    r("write_bytes", FsWrite, Notable, "write",  NoArg),
    r("read_text",   FsRead,  Info,    "read",   NoArg),
    r("read_bytes",  FsRead,  Info,    "read",   NoArg),
    r("iterdir",     FsRead,  Info,    "list",   NoArg),
    r("rglob",       FsRead,  Info,    "glob",   Pos(0)),
    rn("unlink",     FsDelete, Caution, "delete", NoArg, "removes a file from disk"),
    rn("rmdir",      FsDelete, Caution, "rmdir",  NoArg, "removes a directory"),
];

fn method_index() -> &'static HashMap<&'static str, &'static Rule> {
    static IDX: OnceLock<HashMap<&'static str, &'static Rule>> = OnceLock::new();
    IDX.get_or_init(|| METHOD_RULES.iter().map(|rule| (rule.path, rule)).collect())
}

/// Look up a bare method name. Only consulted once the fully-qualified lookup
/// has failed, so a resolved receiver always wins.
pub fn method_lookup(method: &str) -> Option<&'static Rule> {
    method_index().get(method).copied()
}

fn index() -> &'static HashMap<&'static str, &'static Rule> {
    static IDX: OnceLock<HashMap<&'static str, &'static Rule>> = OnceLock::new();
    IDX.get_or_init(|| RULES.iter().map(|rule| (rule.path, rule)).collect())
}

pub fn lookup(path: &str) -> Option<&'static Rule> {
    index().get(path).copied()
}

/// Modules whose calls count as "compute" when no more specific rule matched.
/// These are the libraries whose presence tells you the snippet is doing
/// numerical work, which is worth showing even without a per-symbol rule.
pub const COMPUTE_MODULES: &[&str] = &[
    "numpy",
    "pandas",
    "scipy",
    "torch",
    "tensorflow",
    "jax",
    "sklearn",
    "statsmodels",
    "polars",
    "xarray",
    "sympy",
    "numba",
    "cupy",
    "dask",
    "matplotlib",
    "seaborn",
    "plotly",
    "networkx",
    "MDAnalysis",
    "mdtraj",
    "Bio",
    "openmm",
    "ase",
    "rdkit",
];

/// Modules that ship with CPython. Anything else needs installing, which is
/// itself worth surfacing when an LLM hands you a snippet to run.
pub const STDLIB: &[&str] = &[
    "abc",
    "argparse",
    "array",
    "ast",
    "asyncio",
    "base64",
    "binascii",
    "bisect",
    "builtins",
    "bz2",
    "calendar",
    "cmath",
    "cmd",
    "collections",
    "colorsys",
    "concurrent",
    "configparser",
    "contextlib",
    "copy",
    "csv",
    "ctypes",
    "dataclasses",
    "datetime",
    "decimal",
    "difflib",
    "dis",
    "email",
    "enum",
    "errno",
    "faulthandler",
    "filecmp",
    "fileinput",
    "fnmatch",
    "fractions",
    "ftplib",
    "functools",
    "gc",
    "getpass",
    "gettext",
    "glob",
    "graphlib",
    "gzip",
    "hashlib",
    "heapq",
    "hmac",
    "html",
    "http",
    "imaplib",
    "importlib",
    "inspect",
    "io",
    "ipaddress",
    "itertools",
    "json",
    "keyword",
    "linecache",
    "locale",
    "logging",
    "lzma",
    "marshal",
    "math",
    "mimetypes",
    "mmap",
    "multiprocessing",
    "netrc",
    "numbers",
    "operator",
    "os",
    "pathlib",
    "pickle",
    "pickletools",
    "pkgutil",
    "platform",
    "plistlib",
    "poplib",
    "posixpath",
    "pprint",
    "profile",
    "pty",
    "queue",
    "quopri",
    "random",
    "re",
    "readline",
    "reprlib",
    "resource",
    "runpy",
    "sched",
    "secrets",
    "select",
    "selectors",
    "shelve",
    "shlex",
    "shutil",
    "signal",
    "site",
    "smtplib",
    "socket",
    "socketserver",
    "sqlite3",
    "ssl",
    "stat",
    "statistics",
    "string",
    "stringprep",
    "struct",
    "subprocess",
    "sys",
    "sysconfig",
    "tarfile",
    "tempfile",
    "termios",
    "textwrap",
    "threading",
    "time",
    "timeit",
    "tkinter",
    "token",
    "tokenize",
    "tomllib",
    "trace",
    "traceback",
    "tracemalloc",
    "tty",
    "types",
    "typing",
    "unicodedata",
    "unittest",
    "urllib",
    "uuid",
    "venv",
    "warnings",
    "wave",
    "weakref",
    "webbrowser",
    "wsgiref",
    "xml",
    "xmlrpc",
    "zipapp",
    "zipfile",
    "zipimport",
    "zlib",
    "zoneinfo",
    "__future__",
];

pub fn is_stdlib(module: &str) -> bool {
    let root = module.split('.').next().unwrap_or(module);
    STDLIB.contains(&root)
}

pub fn is_compute_module(module: &str) -> bool {
    let root = module.split('.').next().unwrap_or(module);
    COMPUTE_MODULES.contains(&root)
}

/// The value kind a call of `path` produces, so later method calls on the
/// result can be resolved. `open(p)` yields a `<file>`, `Path(p)` a `<path>`.
pub fn produces(path: &str) -> ValueKind {
    match path {
        "open" | "io.open" | "gzip.open" | "bz2.open" | "lzma.open" | "codecs.open" => {
            ValueKind::File
        }
        "tempfile.NamedTemporaryFile" | "tempfile.TemporaryFile" => ValueKind::File,
        "pathlib.Path" | "pathlib.PurePath" | "pathlib.PosixPath" => ValueKind::Path,
        "requests.Session"
        | "httpx.Client"
        | "httpx.AsyncClient"
        | "aiohttp.ClientSession"
        | "urllib3.PoolManager" => ValueKind::Session,
        "requests.get"
        | "requests.post"
        | "requests.put"
        | "requests.patch"
        | "requests.delete"
        | "requests.head"
        | "requests.request"
        | "httpx.get"
        | "httpx.post"
        | "urllib.request.urlopen" => ValueKind::Response,
        "subprocess.run" | "subprocess.Popen" => ValueKind::Process,
        "socket.socket" | "socket.create_connection" => ValueKind::Socket,
        "sqlite3.connect" | "psycopg2.connect" | "pymysql.connect" => ValueKind::Connection,
        "pandas.DataFrame"
        | "pandas.read_csv"
        | "pandas.read_parquet"
        | "pandas.read_json"
        | "pandas.read_excel"
        | "pandas.read_hdf"
        | "pandas.read_table"
        | "pandas.read_sql"
        | "polars.read_csv"
        | "polars.DataFrame" => ValueKind::Frame,
        "numpy.array" | "numpy.zeros" | "numpy.ones" | "numpy.arange" | "numpy.linspace"
        | "numpy.empty" | "numpy.load" | "numpy.loadtxt" | "numpy.genfromtxt" | "numpy.asarray"
        | "numpy.full" | "numpy.eye" => ValueKind::Array,
        "torch.tensor" | "torch.zeros" | "torch.ones" | "torch.randn" | "torch.from_numpy"
        | "torch.load" => ValueKind::Tensor,
        "str" | "repr" | "format" | "input" => ValueKind::Text,
        "int" | "float" | "len" | "sum" | "abs" | "round" | "min" | "max" | "ord" => {
            ValueKind::Number
        }
        "bool" | "isinstance" | "hasattr" | "callable" | "any" | "all" => ValueKind::Bool,
        "list" | "sorted" | "tuple" | "set" | "range" | "enumerate" | "zip" | "map" | "filter"
        | "reversed" | "glob.glob" | "os.listdir" => ValueKind::Seq,
        "dict"
        | "json.load"
        | "json.loads"
        | "collections.Counter"
        | "collections.defaultdict"
        | "collections.OrderedDict" => ValueKind::Map,
        _ => ValueKind::Unknown,
    }
}

/// Namespace prefix used to key method rules for an inferred value kind.
pub fn kind_prefix(kind: ValueKind) -> Option<&'static str> {
    match kind {
        ValueKind::File => Some("<file>"),
        ValueKind::Path => Some("<path>"),
        ValueKind::Session => Some("<session>"),
        ValueKind::Response => Some("<response>"),
        ValueKind::Frame => Some("<frame>"),
        ValueKind::Process => Some("<proc>"),
        ValueKind::Socket => Some("<socket>"),
        ValueKind::Connection => Some("<conn>"),
        _ => None,
    }
}

/// Shell fragments and paths that turn an ordinary command into one worth
/// reading twice. Matched case-insensitively against string literals that
/// reach a process- or filesystem-flavoured call.
pub const HAZARD_STRINGS: &[(&str, &str)] = &[
    ("rm -rf", "recursive delete"),
    ("rm -fr", "recursive delete"),
    ("sudo ", "elevates privileges"),
    ("chmod 777", "makes a path world-writable"),
    ("mkfs", "formats a filesystem"),
    ("dd if=", "raw device write"),
    ("| sh", "pipes downloaded content into a shell"),
    ("| bash", "pipes downloaded content into a shell"),
    ("curl ", "downloads from the network"),
    ("wget ", "downloads from the network"),
    ("/etc/passwd", "reads a system credential file"),
    ("/etc/shadow", "reads a system credential file"),
    (".ssh/", "touches SSH keys"),
    (".aws/credentials", "touches cloud credentials"),
    ("~/.netrc", "touches stored credentials"),
    ("pip install", "installs packages"),
    ("apt-get", "changes system packages"),
    ("git push", "publishes to a remote"),
    ("--force", "forces an otherwise-refused operation"),
];

/// Argument names whose presence changes how a call should be read.
pub fn kwarg_hazard(callee: &str, kw: &str, truthy: bool) -> Option<(&'static str, Severity)> {
    match (callee, kw, truthy) {
        (c, "shell", true) if c.starts_with("subprocess.") => Some((
            "shell=True — the argument is interpreted by /bin/sh",
            Severity::Caution,
        )),
        (c, "check", false) if c.starts_with("subprocess.") => Some((
            "check=False — a failing command is ignored",
            Severity::Notable,
        )),
        ("requests.get" | "requests.post" | "requests.request", "verify", false) => Some((
            "verify=False — TLS certificates are not checked",
            Severity::Caution,
        )),
        (_, "ignore_errors", true) => Some((
            "ignore_errors=True — failures pass silently",
            Severity::Notable,
        )),
        (_, "exist_ok", true) => None,
        _ => None,
    }
}
