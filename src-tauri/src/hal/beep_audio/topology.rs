use anyhow::{Context, Result, ensure};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Default)]
pub(super) struct Part {
    pub name: String,
    pub mute: bool,
    pub volume: bool,
    pub gain: bool,
    pub terminal: bool,
    pub sum: bool,
    pub incoming: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Selection {
    pub name: String,
    pub mute: Option<String>,
    pub volume: Option<String>,
}

pub(super) fn select(root: &str, parts: &BTreeMap<String, Part>) -> Result<Option<Selection>> {
    let mut groups = vec![Selection::default()];
    let mut walking = Walk {
        parts,
        groups: &mut groups,
        path: BTreeSet::new(),
        visits: 0,
    };
    walking.visit(root, Some(0))?;
    Ok(groups.into_iter().find(|group| group.name == "PC Beep"))
}

struct Walk<'a> {
    parts: &'a BTreeMap<String, Part>,
    groups: &'a mut Vec<Selection>,
    path: BTreeSet<String>,
    visits: usize,
}
impl Walk<'_> {
    fn visit(&mut self, id: &str, mut group: Option<usize>) -> Result<()> {
        self.visits += 1;
        ensure!(
            self.visits <= 1024 && self.path.len() < 128,
            "原厂音频拓扑遍历超出可核对范围"
        );
        ensure!(self.path.insert(id.into()), "原厂音频拓扑存在循环");
        let part = self.parts.get(id).context("原厂音频拓扑缺少上游节点")?;
        if !part.terminal {
            if part.mute || part.volume || part.gain {
                let slot = *group.get_or_insert_with(|| {
                    self.groups.push(Selection::default());
                    self.groups.len() - 1
                });
                let selected = &mut self.groups[slot];
                if part.mute {
                    selected.mute = Some(id.into());
                }
                if part.volume {
                    selected.name = part.name.clone();
                    selected.volume = Some(id.into());
                }
            }
            let upstream = part.incoming.clone();
            let next = if upstream.len() > 1 || part.sum {
                None
            } else {
                group
            };
            for incoming in upstream {
                self.visit(&incoming, next)?;
            }
        }
        self.path.remove(id);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
