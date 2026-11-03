//! File trees. A folder shows its name until you `cd` in, which is the draw.

use rand::seq::SliceRandom;
use rand::Rng;

use crate::catalog::{Kind, Levels};

#[derive(Clone, Debug)]
pub struct File {
    pub name: String,
    pub kind: Kind,
    pub depth: u8,
    pub chunks: u8,
    pub taken: u8,
    pub wiped: bool,
    pub gone: bool,
    pub banked: bool,
    pub doubled: bool,
}

#[derive(Clone, Debug)]
pub struct Folder {
    pub name: String,
    pub depth: u8,
    pub locked: u8,
    pub was_locked: bool,
    pub revealed: bool,
    pub visited: bool,
    pub kids: Vec<Node>,
}

#[derive(Clone, Debug)]
pub enum Node {
    Dir(Folder),
    File(File),
}

impl Node {
    pub fn name(&self) -> &str {
        match self {
            Self::Dir(d) => &d.name,
            Self::File(f) => &f.name,
        }
    }
}

impl Folder {
    pub fn new(name: &str, depth: u8) -> Self {
        Self {
            name: name.to_string(),
            depth,
            locked: 0,
            was_locked: false,
            revealed: false,
            visited: false,
            kids: Vec::new(),
        }
    }

    pub fn lock(mut self, n: u8) -> Self {
        self.locked = n;
        self.was_locked = n > 0;
        self
    }

    pub fn file_count(&self) -> usize {
        self.kids
            .iter()
            .filter(|k| matches!(k, Node::File(_)))
            .count()
    }

    pub fn is_looted(&self) -> bool {
        if !self.visited {
            return false;
        }
        self.kids.iter().all(|k| match k {
            Node::File(f) => {
                if f.gone {
                    true
                } else if f.kind == Kind::Log {
                    f.wiped
                } else {
                    f.banked
                }
            }
            Node::Dir(d) => d.locked == 0 && d.is_looted(),
        })
    }
}

pub fn make_file(name: &str, kind: Kind, depth: u8, levels: Levels) -> File {
    let chunks = match kind {
        Kind::Data => 2,
        Kind::Key | Kind::Cred | Kind::Log => 1,
        Kind::Zero => {
            if levels.zero >= 2 {
                3
            } else {
                4
            }
        }
    };
    File {
        name: name.to_string(),
        kind,
        depth,
        chunks,
        taken: 0,
        wiped: false,
        gone: false,
        banked: false,
        doubled: false,
    }
}

fn take_name(rng: &mut impl Rng, pool: &[&str], used: &mut Vec<String>) -> String {
    let mut choices: Vec<&str> = pool
        .iter()
        .copied()
        .filter(|n| !used.iter().any(|u| u == n))
        .collect();
    if choices.is_empty() {
        choices = pool.to_vec();
    }
    let name = choices.choose(rng).copied().unwrap_or(pool[0]);
    used.push(name.to_string());
    name.to_string()
}

fn push_file(
    folder: &mut Folder,
    rng: &mut impl Rng,
    kind: Kind,
    used: &mut Vec<String>,
    levels: Levels,
) {
    let pool: &[&str] = match kind {
        Kind::Data => &[
            "notes.txt",
            "homework.doc",
            "photos.tar",
            "backup.tar",
            "cache.bin",
            "grades.xls",
            "readme.txt",
            "mail.db",
            "recipes.txt",
            "dump.bin",
        ],
        Kind::Key => &["id_rsa", "id_rsa.pub", "shadow", "api.key", "token.pem"],
        Kind::Cred => &["wifi.txt", "cookies.txt", ".netrc", "vpn.conf"],
        Kind::Log => &[
            "auth.log",
            "syslog",
            "bash_history",
            "cron.log",
            "chat.log",
            "kern.log",
        ],
        Kind::Zero => &["zeroday.bin"],
    };
    let name = if kind == Kind::Zero {
        "zeroday.bin".to_string()
    } else {
        take_name(rng, pool, used)
    };
    let mut file = make_file(&name, kind, folder.depth.max(1), levels);
    if kind == Kind::Data && folder.depth >= 2 && rng.gen_bool(0.25) {
        file.chunks = 3;
    }
    folder.kids.push(Node::File(file));
}

fn push_kinds(
    folder: &mut Folder,
    rng: &mut impl Rng,
    kinds: &[Kind],
    used: &mut Vec<String>,
    levels: Levels,
) {
    for kind in kinds {
        push_file(folder, rng, *kind, used, levels);
    }
}

pub fn tutorial_tree() -> Folder {
    let mut root = Folder::new("you", 0);
    root.revealed = true;
    root.visited = true;
    let levels = Levels::default();
    let mut downloads = Folder::new("downloads", 1);
    downloads
        .kids
        .push(Node::File(make_file("readme.txt", Kind::Data, 1, levels)));
    let mut games = Folder::new("games", 2);
    games
        .kids
        .push(Node::File(make_file("photos.tar", Kind::Data, 2, levels)));
    let mut ssh = Folder::new(".ssh", 2).lock(1);
    ssh.kids
        .push(Node::File(make_file("id_rsa", Kind::Key, 2, levels)));
    root.kids.push(Node::Dir(downloads));
    root.kids
        .push(Node::File(make_file("homework.doc", Kind::Data, 1, levels)));
    root.kids
        .push(Node::File(make_file("chat.log", Kind::Log, 1, levels)));
    root.kids.push(Node::Dir(games));
    root.kids.push(Node::Dir(ssh));
    root
}

/// L1 B2: three open folders, enough DATA and a KEY to clear 120 with care.
pub fn gen_b2(rng: &mut impl Rng, levels: Levels) -> Folder {
    let mut used = Vec::new();
    let mut folders = Vec::new();
    let plans: [&[Kind]; 3] = [
        &[Kind::Key, Kind::Data, Kind::Log],
        &[Kind::Data, Kind::Log, Kind::Cred],
        &[Kind::Data, Kind::Data],
    ];
    let mut names = ["home", "tmp", "opt", "mail", "docs", "var"];
    names.shuffle(rng);
    for (i, plan) in plans.iter().enumerate() {
        let mut folder = Folder::new(names[i], 1);
        push_kinds(&mut folder, rng, plan, &mut used, levels);
        folders.push(folder);
    }
    folders.shuffle(rng);
    let mut root = Folder::new("kid", 0);
    root.revealed = true;
    root.visited = true;
    for folder in folders {
        root.kids.push(Node::Dir(folder));
    }
    root
}

/// L1 B3, THE PARENT. Every depth-1 folder has a LOG. Vault is optional.
pub fn gen_b3(rng: &mut impl Rng, levels: Levels) -> Folder {
    let mut used = Vec::new();
    let plans: [(&str, &[Kind]); 3] = [
        ("home", &[Kind::Log, Kind::Data, Kind::Key]),
        ("var", &[Kind::Log, Kind::Cred, Kind::Data]),
        ("srv", &[Kind::Log, Kind::Data]),
    ];
    let mut root = Folder::new("kid", 0);
    root.revealed = true;
    root.visited = true;
    for (name, plan) in plans {
        let mut folder = Folder::new(name, 1);
        push_kinds(&mut folder, rng, plan, &mut used, levels);
        root.kids.push(Node::Dir(folder));
    }
    let mut etc = Folder::new("etc", 2).lock(1);
    push_kinds(&mut etc, rng, &[Kind::Key, Kind::Data], &mut used, levels);
    let mut vault = Folder::new("root", 3).lock(1);
    push_file(&mut vault, rng, Kind::Zero, &mut used, levels);
    push_file(&mut vault, rng, Kind::Data, &mut used, levels);
    root.kids.push(Node::Dir(etc));
    root.kids.push(Node::Dir(vault));
    root
}

pub fn reveal_all(folder: &mut Folder) {
    folder.revealed = true;
    for kid in &mut folder.kids {
        if let Node::Dir(dir) = kid {
            reveal_all(dir);
        }
    }
}

pub fn count_unwiped_logs(folder: &Folder) -> u32 {
    let mut n = 0;
    for kid in &folder.kids {
        match kid {
            Node::File(f) if f.kind == Kind::Log && !f.wiped && !f.gone => n += 1,
            Node::Dir(d) => n += count_unwiped_logs(d),
            _ => {}
        }
    }
    n
}

pub fn folder_names(folder: &Folder, out: &mut Vec<String>) {
    if !folder.name.is_empty() && folder.depth > 0 {
        out.push(folder.name.clone());
    }
    for kid in &folder.kids {
        if let Node::Dir(d) = kid {
            folder_names(d, out);
        }
    }
}
