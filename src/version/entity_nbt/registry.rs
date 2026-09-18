use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::Path;

use super::scan::{ClassInfo, parse_string};
use super::{EntityTagType, is_identifier};

/// `EntityTypeIds.java`：常量名 → 实体 id。
pub(super) fn read_entity_ids(path: &Path) -> Result<BTreeMap<String, String>, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("无法读取 {}：{error}", path.display()))?;
    let mut ids = BTreeMap::new();
    let mut search = 0;
    while let Some(offset) = text[search..].find("create(\"") {
        let index = search + offset;
        search = index + "create(\"".len();
        let Some((id, _)) = parse_string(&text, index + "create(".len()) else {
            continue;
        };
        let name = text[..index]
            .trim_end()
            .trim_end_matches('=')
            .trim_end()
            .rsplit(|character: char| !is_identifier(character))
            .next()
            .unwrap_or_default()
            .to_owned();
        if !name.is_empty() {
            ids.insert(name, id);
        }
    }
    Ok(ids)
}

/// `EntityTypes.java`：`EntityType<类> 常量 = register(EntityTypeIds.常量,`。
pub(super) fn read_entity_registrations(path: &Path) -> Result<Vec<(String, String)>, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("无法读取 {}：{error}", path.display()))?;
    let mut registrations = Vec::new();
    for line in text.lines() {
        let Some(register) = line.find("register(EntityTypeIds.") else {
            continue;
        };
        let Some(generic) = line.find("EntityType<") else {
            continue;
        };
        let type_start = generic + "EntityType<".len();
        let Some(type_end) = line[type_start..].find('>') else {
            continue;
        };
        let class = line[type_start..type_start + type_end]
            .rsplit('.')
            .next()
            .unwrap_or_default()
            .trim()
            .to_owned();
        let id_start = register + "register(EntityTypeIds.".len();
        let id_length = line[id_start..]
            .char_indices()
            .take_while(|(_, character)| is_identifier(*character))
            .last()
            .map(|(offset, character)| offset + character.len_utf8())
            .unwrap_or(0);
        let identifier = line[id_start..id_start + id_length].to_owned();
        if !class.is_empty() && !identifier.is_empty() {
            registrations.push((identifier, class));
        }
    }
    Ok(registrations)
}

/// 类沿继承链能写出的全部键。
pub(super) fn inherited_tags(
    classes: &BTreeMap<String, ClassInfo>,
    class: &str,
    resolved: &mut BTreeSet<String>,
    visiting: &mut HashSet<String>,
) {
    let Some(info) = classes.get(class) else {
        return;
    };
    if visiting.contains(class) {
        return;
    }
    visiting.insert(class.to_owned());
    for key in info.tags.keys() {
        resolved.insert(key.clone());
    }
    if let Some(parent) = &info.parent {
        inherited_tags(classes, parent, resolved, visiting);
    }
}

pub(super) fn all_class_tags(
    classes: &BTreeMap<String, ClassInfo>,
    class: &str,
) -> Vec<(String, EntityTagType)> {
    let mut resolved = BTreeSet::new();
    inherited_tags(classes, class, &mut resolved, &mut HashSet::new());
    let mut categories = Vec::new();
    for key in resolved {
        let category = inherited_category(classes, class, &key).unwrap_or(EntityTagType::Any);
        categories.push((key, category));
    }
    categories
}

fn inherited_category(
    classes: &BTreeMap<String, ClassInfo>,
    class: &str,
    key: &str,
) -> Option<EntityTagType> {
    let info = classes.get(class)?;
    if let Some(category) = info.tags.get(key) {
        return Some(*category);
    }
    info.parent
        .as_deref()
        .and_then(|parent| inherited_category(classes, parent, key))
}

pub(super) fn collect_java_files(
    root: &Path,
    files: &mut Vec<std::path::PathBuf>,
) -> Result<(), String> {
    let entries =
        fs::read_dir(root).map_err(|error| format!("无法读取 {}：{error}", root.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("无法读取目录项：{error}"))?;
        let path = entry.path();
        if path.is_dir() {
            collect_java_files(&path, files)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension == "java")
        {
            files.push(path);
        }
    }
    Ok(())
}
