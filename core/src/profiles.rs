//! The characters of an instance read from its profile files (no server needed): username, nickname, side, level,
//! edition and roubles on hand, for the character capsule.

use crate::instance::Instance;
use crate::util::*;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct Character {
    pub profile_id: String,
    pub username: String,
    pub nickname: String,
    pub side: String,
    pub level: i64,
    pub edition: String,
    pub roubles: i64,
    /// Set up in game (a new profile has no PMC yet)
    pub started: bool,
}

const ROUBLES: &str = "5449016a4bdc2d6f028b456f";

pub fn read_profile(v: &Value) -> Character {
    let info = v.get("info").cloned().unwrap_or(Value::Null);
    let s = |o: &Value, k: &str| o.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let pmc = v.pointer("/characters/pmc").cloned().unwrap_or(Value::Null);
    let pinfo = pmc.get("Info").cloned().unwrap_or(Value::Null);
    let roubles = pmc
        .pointer("/Inventory/items")
        .and_then(|i| i.as_array())
        .map(|items| {
            items
                .iter()
                .filter(|i| i.get("_tpl").and_then(|t| t.as_str()) == Some(ROUBLES))
                .map(|i| i.pointer("/upd/StackObjectsCount").and_then(|c| c.as_i64()).unwrap_or(1))
                .sum()
        })
        .unwrap_or(0);
    let nickname = s(&pinfo, "Nickname");
    Character {
        profile_id: s(&info, "id"),
        username: s(&info, "username"),
        started: !nickname.is_empty() && pinfo.get("Level").is_some(),
        nickname,
        side: s(&pinfo, "Side"),
        level: pinfo.get("Level").and_then(|x| x.as_i64()).unwrap_or(0),
        edition: s(&info, "edition"),
        roubles,
    }
}

pub fn list(inst: &Instance) -> Vec<Character> {
    let mut v: Vec<Character> = list_dir(&inst.profiles_dir())
        .into_iter()
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "json"))
        .filter_map(|p| read_json(&p).ok())
        .map(|j| read_profile(&j))
        .filter(|c| !c.profile_id.is_empty())
        .collect();
    v.sort_by(|a, b| b.level.cmp(&a.level).then(a.username.cmp(&b.username)));
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instance::detect;
    use crate::instance::tests::{fake_instance, touch};

    #[test]
    fn reads_characters() {
        let r = fake_instance("prof", "SPT_Runtime");
        touch(&r.join("SPT_Runtime/user/profiles/bbb.json"), br#"{"info":{"id":"bbb","username":"fresh","edition":"Edge Of Darkness"},"characters":{"pmc":{}}}"#);
        let l = list(&detect(&r));
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].nickname, "BEAR_Main");
        assert_eq!(l[0].level, 34);
        assert_eq!(l[0].roubles, 500000);
        assert!(l[0].started);
        assert_eq!(l[1].username, "fresh");
        assert!(!l[1].started);
        let _ = std::fs::remove_dir_all(&r);
    }
}
