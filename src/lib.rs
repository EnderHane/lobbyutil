use std::collections::VecDeque;

use celesteloader::map::{decode::Element, Entity, Map, Room, Trigger};

pub const PNG_MAGIC_STR: &str = "Draw List";

pub fn find_mini_heart_door(mp: &Map) -> Option<(&Entity, &Room)> {
    mp.rooms.iter().find_map(|r| {
        r.find_entity_by_name("CollabUtils2/MiniHeartDoor")
            .map(|e| (e, r))
    })
}

pub fn warps(mp: &Map) -> Vec<(&Entity, &Room)> {
    let mut ws = mp
        .rooms
        .iter()
        .flat_map(|r| {
            r.entities
                .iter()
                .filter(|t| t.name == "CollabUtils2/LobbyMapWarp")
                .map(move |t| (r, t))
        })
        .map(|(r, w)| (w, r))
        .collect::<Vec<_>>();
    ws.sort_by(|(a, ra), (b, rb)| {
        i32::cmp(&a.id.unwrap(), &b.id.unwrap())
            .then(f32::total_cmp(
                &(a.position.0 * a.position.0 + a.position.1 * a.position.1),
                &(b.position.0 * b.position.0 + b.position.1 * b.position.1),
            ))
            .then(str::cmp(&ra.name, &rb.name))
    });
    ws
}

pub fn chapters(mp: &Map) -> Vec<(&Trigger, &Room)> {
    let mut chs = mp
        .rooms
        .iter()
        .flat_map(|r| {
            r.triggers
                .iter()
                .filter(|t| t.name == "CollabUtils2/ChapterPanelTrigger")
                .map(move |t| (r, t))
        })
        .map(|(r, ch)| (ch, r))
        .collect::<Vec<_>>();
    chs.sort_by(|(a, ra), (b, rb)| {
        i32::cmp(&a.id.unwrap(), &b.id.unwrap())
            .then(f32::total_cmp(
                &(a.position.0 * a.position.0 + a.position.1 * a.position.1),
                &(b.position.0 * b.position.0 + b.position.1 * b.position.1),
            ))
            .then(str::cmp(&ra.name, &rb.name))
    });
    chs
}

pub fn default_spwan(room: &Room) -> &Entity {
    room.entities_by_name("player")
        .filter(|e| e.raw.get_attr::<bool>("isDefaultSpawn").unwrap_or(false))
        .chain(room.entities_by_name("player"))
        .nth(0)
        .expect("no spawn!")
}

pub fn start_level<'a>(root: &Element, mp: &'a Map) -> &'a Room {
    traverse_element(&root)
        .find_map(|e| {
            e.attributes
                .get("StartLevel")
                .map(|v| v.get::<&str>().unwrap())
        })
        .map(|n| mp.rooms.iter().find(|r| r.name == n))
        .flatten()
        .or(mp.rooms.get(0))
        .expect("no room!")
}

pub fn traverse_element<'a>(root: &'a Element) -> impl Iterator<Item = &'a Element<'a>> {
    struct ElementIter<'a> {
        queue: VecDeque<&'a Element<'a>>,
    }

    impl<'a> Iterator for ElementIter<'a> {
        type Item = &'a Element<'a>;

        fn next(&mut self) -> Option<Self::Item> {
            self.queue.pop_front().map(|e| {
                self.queue.extend(e.children.iter());
                e
            })
        }
    }

    ElementIter {
        queue: VecDeque::from([root]),
    }
}

pub fn pos_in_room(pos: (f32, f32), room: &Room) -> (f32, f32) {
    (
        pos.0 + room.bounds.position.x as f32,
        pos.1 + room.bounds.position.y as f32,
    )
}

pub fn pos_bounded(pos: (f32, f32), map: &Map) -> (f32, f32) {
    (
        pos.0 - map.bounds().position.x as f32,
        pos.1 - map.bounds().position.y as f32,
    )
}
