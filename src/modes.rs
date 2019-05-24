use std::borrow::Cow;

struct SimpleQuery<'a> {
    modes: &'a [u8],
    value: bool,
}

impl<'a> Iterator for SimpleQuery<'a> {
    type Item = (bool, u8);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.modes.is_empty() {
                return None;
            }
            match self.modes[0] {
                b'+' => { self.value = true; },
                b'-' => { self.value = false; },
                c => {
                    self.modes = &self.modes[1..];
                    return Some((self.value, c));
                },
            }
            self.modes = &self.modes[1..];
        }
    }
}

pub enum Error {
    UnknownMode(char),
    MissingModeParam,
    BadModeParam,
}

pub type Result<T> = std::result::Result<T, Error>;

pub enum UserModeChange {
    Invisible(bool),
    Wallops(bool),
    ServerNotices(bool),
}

pub struct UserQuery<'a> {
    inner: SimpleQuery<'a>,
}

impl<'a> UserQuery<'a> {
    pub fn new(modes: &'a [u8]) -> UserQuery<'a> {
        UserQuery {
            inner: SimpleQuery {
                modes,
                value: true,
            },
        }
    }
}

impl<'a> Iterator for UserQuery<'a> {
    type Item = Result<UserModeChange>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(value, mode)| {
            match mode {
                b'i' => Ok(UserModeChange::Invisible(value)),
                b'w' => Ok(UserModeChange::Wallops(value)),
                b's' => Ok(UserModeChange::ServerNotices(value)),
                other => Err(Error::UnknownMode(other as char)),
            }
        })
    }
}

#[derive(Debug)]
pub enum ChannelModeChange<'a> {
    Anonymous(bool),
    InviteOnly(bool),
    Moderated(bool),
    NoPrivMsgFromOutside(bool),
    Quiet(bool),
    Private(bool),
    Secret(bool),
    TopicRestricted(bool),
    Key(bool, Cow<'a, str>),
    UserLimit(Option<Cow<'a, str>>),
    GetBans,
    GetExceptions,
    GetInvitations,
    ChangeBan(bool, Cow<'a, str>),
    ChangeException(bool, Cow<'a, str>),
    ChangeInvitation(bool, Cow<'a, str>),
    ChangeOperator(bool, Cow<'a, str>),
    ChangeVoice(bool, Cow<'a, str>),
}

impl<'a> ChannelModeChange<'a> {
    pub fn value(&self) -> bool {
        use ChannelModeChange::*;
        match self {
            Anonymous(v) |
            InviteOnly(v) |
            Moderated(v) |
            NoPrivMsgFromOutside(v) |
            Quiet(v) |
            Private(v) |
            Secret(v) |
            TopicRestricted(v) |
            Key(v, _) |
            ChangeBan(v, _) |
            ChangeException(v, _) |
            ChangeInvitation(v, _) |
            ChangeOperator(v, _) |
            ChangeVoice(v, _) => *v,
            UserLimit(l) => l.is_some(),
            _ => false,
        }
    }

    pub fn symbol(&self) -> Option<char> {
        use ChannelModeChange::*;
        match self {
            Anonymous(_) => Some('a'),
            InviteOnly(_) => Some('i'),
            Moderated(_) => Some('m'),
            NoPrivMsgFromOutside(_) => Some('n'),
            Quiet(_) => Some('q'),
            Private(_) => Some('p'),
            Secret(_) => Some('s'),
            TopicRestricted(_) => Some('t'),
            Key(_, _) => Some('k'),
            UserLimit(_) => Some('l'),
            ChangeBan(_, _) => Some('b'),
            ChangeException(_, _) => Some('e'),
            ChangeInvitation(_, _) => Some('I'),
            ChangeOperator(_, _) => Some('o'),
            ChangeVoice(_, _) => Some('v'),
            _ => None,
        }
    }

    pub fn param(&'a self) -> Option<&'a str> {
        use ChannelModeChange::*;
        match self {
            UserLimit(Some(p)) |
            Key(_, p) |
            ChangeBan(_, p) |
            ChangeException(_, p) |
            ChangeInvitation(_, p) |
            ChangeOperator(_, p) |
            ChangeVoice(_, p) => Some(p.as_ref()),
            _ => None,
        }
    }
}

pub struct ChannelQuery<'a, I> {
    inner: SimpleQuery<'a>,
    params: I,
}

impl<'a, I> ChannelQuery<'a, I> {
    pub fn new(modes: &'a str, params: I) -> ChannelQuery<'a, I> {
        let modes = modes.as_bytes();
        ChannelQuery {
            inner: SimpleQuery {
                modes,
                value: true,
            },
            params,
        }
    }
}

impl<'a, I> Iterator for ChannelQuery<'a, I>
    where I: Iterator<Item=&'a str>
{
    type Item = Result<ChannelModeChange<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(value, mode)| {
            match mode {
                b'a' => Ok(ChannelModeChange::Anonymous(value)),
                b'i' => Ok(ChannelModeChange::InviteOnly(value)),
                b'm' => Ok(ChannelModeChange::Moderated(value)),
                b'n' => Ok(ChannelModeChange::NoPrivMsgFromOutside(value)),
                b'q' => Ok(ChannelModeChange::Quiet(value)),
                b'p' => Ok(ChannelModeChange::Private(value)),
                b't' => Ok(ChannelModeChange::TopicRestricted(value)),
                b'k' => if let Some(param) = self.params.next() {
                    Ok(ChannelModeChange::Key(value, param.into()))
                } else {
                    Err(Error::MissingModeParam)
                },
                b'l' => Ok(ChannelModeChange::UserLimit(self.params.next().map(Into::into))),
                b'b' => if let Some(param) = self.params.next() {
                    Ok(ChannelModeChange::ChangeBan(value, param.into()))
                } else {
                    Ok(ChannelModeChange::GetBans)
                },
                b'e' => if let Some(param) = self.params.next() {
                    Ok(ChannelModeChange::ChangeException(value, param.into()))
                } else {
                    Ok(ChannelModeChange::GetExceptions)
                },
                b'I' => if let Some(param) = self.params.next() {
                    Ok(ChannelModeChange::ChangeInvitation(value, param.into()))
                } else {
                    Ok(ChannelModeChange::GetInvitations)
                },
                b'o' => if let Some(param) = self.params.next() {
                    Ok(ChannelModeChange::ChangeOperator(value, param.into()))
                } else {
                    Err(Error::MissingModeParam)
                },
                b'v' => if let Some(param) = self.params.next() {
                    Ok(ChannelModeChange::ChangeVoice(value, param.into()))
                } else {
                    Err(Error::MissingModeParam)
                },
                other => Err(Error::UnknownMode(other as char)),
            }
        })
    }
}
