use std::io;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalAccount {
    pub uid: u32,
    pub gid: u32,
    pub username: String,
    pub gecos: String,
    pub home: String,
    pub shell: String,
}

impl LocalAccount {
    pub fn login_name(&self) -> &str {
        &self.username
    }

    pub fn display_name(&self) -> &str {
        self.gecos
            .split(',')
            .next()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(&self.username)
    }

    pub fn login_allowed(&self) -> bool {
        self.uid >= 1000
            && !self.shell.is_empty()
            && self.shell != "/sbin/nologin"
            && self.shell != "/bin/false"
    }
}

#[derive(Clone, Debug, Default)]
pub struct AccountDirectory {
    accounts: Vec<LocalAccount>,
}

impl AccountDirectory {
    pub fn discover() -> io::Result<Self> {
        let passwd = std::fs::read_to_string("/etc/passwd")?;
        Ok(Self::from_passwd(&passwd))
    }

    pub fn from_passwd(passwd: &str) -> Self {
        let mut accounts = passwd
            .lines()
            .filter_map(parse_passwd_line)
            .filter(LocalAccount::login_allowed)
            .collect::<Vec<_>>();
        accounts.sort_by(|left, right| left.username.cmp(&right.username));
        Self { accounts }
    }

    pub fn accounts(&self) -> &[LocalAccount] {
        &self.accounts
    }

    pub fn first(&self) -> Option<&LocalAccount> {
        self.accounts.first()
    }

    pub fn lookup(&self, username: &str) -> Option<&LocalAccount> {
        self.accounts
            .iter()
            .find(|account| account.username == username)
    }
}

fn parse_passwd_line(line: &str) -> Option<LocalAccount> {
    let mut fields = line.splitn(7, ':');
    let username = fields.next()?.trim();
    let _password = fields.next()?;
    let uid = fields.next()?.parse().ok()?;
    let gid = fields.next()?.parse().ok()?;
    let gecos = fields.next()?.to_owned();
    let home = fields.next()?.to_owned();
    let shell = fields.next()?.to_owned();
    if username.is_empty() {
        return None;
    }
    Some(LocalAccount {
        uid,
        gid,
        username: username.to_owned(),
        gecos,
        home,
        shell,
    })
}

#[cfg(test)]
mod tests {
    use super::AccountDirectory;

    #[test]
    fn login_directory_excludes_system_and_nologin_accounts() {
        let passwd = concat!(
            "root:x:0:0:root:/root:/bin/sh\n",
            "daemon:x:2:2:daemon:/sbin:/sbin/nologin\n",
            "alice:x:1000:100:Alice:/home/alice:/bin/ash\n",
            "bob:x:1001:100:Bob:/home/bob:/bin/false\n",
            "carol:x:1002:100:Carol:/home/carol:/bin/ash\n",
        );
        let directory = AccountDirectory::from_passwd(passwd);
        assert_eq!(directory.accounts().len(), 2);
        assert_eq!(directory.accounts()[0].username, "alice");
        assert_eq!(directory.accounts()[1].username, "carol");
    }
}
