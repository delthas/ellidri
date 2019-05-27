use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

#[cfg(feature = "irdille")]
use regex::Regex;

use crate::message::{MessageBuffer, Reply, rpl};
use crate::modes;

/// Modes applied to clients on a per-channel basis.
///
/// https://tools.ietf.org/html/rfc2811.html#section-4.1
#[derive(Default)]
pub struct MemberModes {
    pub creator: bool,
    pub operator: bool,
    pub voice: bool,
}

impl MemberModes {
    pub fn symbol(&self) -> Option<char> {
        if self.operator {
            Some('@')
        } else if self.voice {
            Some('+')
        } else {
            None
        }
    }
}

/// Channel data.
#[derive(Default)]
pub struct Channel {
    /// Set of channel members, identified by their socket address, and associated with their
    /// channel mode.
    pub members: HashMap<SocketAddr, MemberModes>,

    /// The topic.
    pub topic: Option<String>,

    pub user_limit: Option<usize>,
    pub key: Option<String>,

    // https://tools.ietf.org/html/rfc2811.html#section-4.3
    pub ban_mask: HashSet<String>,
    pub exception_mask: HashSet<String>,
    pub invitation_mask: HashSet<String>,

    // Modes: https://tools.ietf.org/html/rfc2811.html#section-4.2
    pub anonymous: bool,
    pub invite_only: bool,
    pub moderated: bool,
    pub no_privmsg_from_outside: bool,
    pub quiet: bool,
    pub secret: bool,
    pub reop: bool,
    pub topic_restricted: bool,

    #[cfg(feature = "irdille")]
    pub msg_modifier: Vec<(f64, Regex, String)>,
}

impl Channel {
    /// Creates a channel with the 'n' mode set.
    pub fn new(modes: &str) -> Channel {
        let mut chan = Channel::default();
        for change in modes::ChannelQuery::simple(modes).filter_map(Result::ok) {
            chan.apply_mode_change(change, |_| "").unwrap();
        }
        chan
    }

    /// Adds a member with the default mode.
    pub fn add_member(&mut self, addr: SocketAddr) {
        let modes = if self.members.is_empty() {
            MemberModes {
                creator: true,
                operator: true,
                voice: false,
            }
        } else {
            MemberModes::default()
        };
        self.members.insert(addr, modes);
    }

    /// Removes a member.
    pub fn remove_member(&mut self, addr: SocketAddr) {
        self.members.remove(&addr);
    }

    pub fn list_entry(&self, msg: MessageBuffer) {
        msg.param(self.members.len().to_string())
            .trailing_param(self.topic.as_ref().map(|s| s.as_ref()).unwrap_or(""));
    }

    pub fn is_banned(&self, nick: &str) -> bool {
        self.ban_mask.contains(nick)
            && !self.exception_mask.contains(nick)
            && !self.invitation_mask.contains(nick)
    }

    pub fn is_invited(&self, nick: &str) -> bool {
        !self.invite_only || self.invitation_mask.contains(nick)
    }

    pub fn can_talk(&self, addr: SocketAddr) -> bool {
        if self.moderated {
            self.members.get(&addr).map(|m| m.voice || m.operator).unwrap_or(false)
        } else {
            !self.no_privmsg_from_outside || self.members.contains_key(&addr)
        }
    }

    pub fn modes(&self, mut out: MessageBuffer, full_info: bool) {
        let modes = out.raw_param();
        modes.push('+');
        if self.anonymous { modes.push('a'); }
        if self.invite_only { modes.push('i'); }
        if self.moderated { modes.push('m'); }
        if self.no_privmsg_from_outside { modes.push('n'); }
        if self.quiet { modes.push('q'); }
        if self.reop { modes.push('r'); }
        if self.secret { modes.push('s'); }
        if self.topic_restricted { modes.push('t'); }
        if self.user_limit.is_some() { modes.push('l'); }
        if self.key.is_some() { modes.push('k'); }

        #[cfg(feature = "irdille")] {
            if !self.msg_modifier.is_empty() { modes.push('P'); }
        }

        if full_info {
            if let Some(user_limit) = self.user_limit {
                out = out.param(user_limit.to_string());
            }
            if let Some(ref key) = self.key {
                out = out.param(key.to_owned());
            }
        }
        out.build();
    }

    pub fn apply_mode_change<'a, F>(&mut self, change: modes::ChannelModeChange,
                                    nick_of: F) -> Result<bool, Reply>
        where F: Fn(&SocketAddr) -> &'a str
    {
        use modes::ChannelModeChange::*;
        let mut applied = false;
        match change {
            Anonymous(value) => {
                applied = self.anonymous != value;
                self.anonymous = value;
            },
            InviteOnly(value) => {
                applied = self.invite_only != value;
                self.invite_only = value;
            },
            Moderated(value) => {
                applied = self.moderated != value;
                self.moderated = value;
            },
            NoPrivMsgFromOutside(value) => {
                applied = self.no_privmsg_from_outside != value;
                self.no_privmsg_from_outside = value;
            },
            Quiet(value) => {
                applied = self.quiet != value;
                self.quiet = value;
            },
            Secret(value) => {
                applied = self.secret != value;
                self.secret = value;
            },
            TopicRestricted(value) => {
                applied = self.topic_restricted != value;
                self.topic_restricted = value;
            },
            Key(value, key) => if value {
                if self.key.is_some() {
                    return Err(rpl::ERR_KEYSET);
                } else {
                    applied = true;
                    self.key = Some(key.to_owned());
                }
            } else if let Some(ref chan_key) = self.key {
                if key == chan_key {
                    applied = true;
                    self.key = None;
                }
            },
            UserLimit(Some(s)) => if let Ok(limit) = s.parse() {
                applied = self.user_limit.map_or(true, |chan_limit| chan_limit != limit);
                self.user_limit = Some(limit);
            },
            UserLimit(None) => {
                applied = self.user_limit.is_some();
                self.user_limit = None;
            },
            ChangeBan(value, param) => {
                applied = if value {
                    self.ban_mask.insert(param.to_owned())
                } else {
                    self.ban_mask.remove(param)
                };
            },
            ChangeException(value, param) => {
                applied = if value {
                    self.exception_mask.insert(param.to_owned())
                } else {
                    self.exception_mask.remove(param)
                };
            },
            ChangeInvitation(value, param) => {
                applied = if value {
                    self.invitation_mask.insert(param.to_owned())
                } else {
                    self.invitation_mask.remove(param)
                };
            },
            ChangeOperator(value, param) => {
                let mut has_it = false;
                for (member, modes) in self.members.iter_mut() {
                    if nick_of(member) == param {
                        has_it = true;
                        applied = modes.operator != value;
                        modes.operator = value;
                        break;
                    }
                }
                if !has_it {
                    return Err(rpl::ERR_USERNOTINCHANNEL);
                }
            },
            ChangeVoice(value, param) => {
                let mut has_it = false;
                for (member, modes) in self.members.iter_mut() {
                    if nick_of(member) == param {
                        has_it = true;
                        applied = modes.operator != value;
                        modes.operator = value;
                        break;
                    }
                }
                if !has_it {
                    return Err(rpl::ERR_USERNOTINCHANNEL);
                }
            },

            #[cfg(feature = "irdille")]
            MsgModifier(Some(p)) => {
                applied = true;
                let mut modifiers = Vec::new();
                let mut split = p.split("||");
                loop {
                    let proba = if let Some(p) = split.next().and_then(|s| s.parse().ok()) {
                        if 0.0 <= p && p <= 1.0 {
                            p
                        } else {
                            break;
                        }
                    } else {
                        break;
                    };
                    let regex = if let Some(r) = split.next().and_then(|s| Regex::new(s).ok()) {
                        r
                    } else {
                        break;
                    };
                    let repl = if let Some(r) = split.next() {
                        r.to_owned()
                    } else {
                        break;
                    };
                    modifiers.push((proba, regex, repl));
                }
                self.msg_modifier = modifiers;
            },
            #[cfg(feature = "irdille")]
            MsgModifier(None) => {
                applied = !self.msg_modifier.is_empty();
                self.msg_modifier = Vec::new();
            },

            _ => {},
        }
        Ok(applied)
    }

    pub fn symbol(&self) -> &'static str {
        if self.secret {
            "@"
        } else {
            "="
        }
    }
}
