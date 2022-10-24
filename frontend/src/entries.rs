use std::collections::HashSet;
use std::ops::Deref;

use dioxus::fermi::hooks::*;
use dioxus::prelude::*;
use futures::{SinkExt, StreamExt};
use gloo_worker::WorkerBridge;
use im::Vector;

use log::{trace, warn};

use model::entry::*;
use model::events::*;
use model::feed::*;

use crate::WORKER_BRIDGE;
use worker::{BackendWorker, WorkerRequest, WorkerResponse};

// We could've stored entries in the context, but in this case
// it would be accessible as a raw value from all the other components
// breaking the action based encapsulation.
// Probably could've had it stored locally, inside the component, but not sure it would work
static ENTRIES: AtomRef<DisplayedEntries> = |_| DisplayedEntries::default();
static MAX_READ_ENTRIES: usize = 20;

#[derive(PartialEq, Eq, Clone)]
struct Entry {
    stored: StoredEntry,
    unread: bool,
}

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let p = self.stored.published.partial_cmp(&other.stored.published);
        if let Some(p) = p {
            if p.is_eq() {
                return self
                    .stored
                    .entry_id
                    .0
                    .partial_cmp(&other.stored.entry_id.0)
                    .map(|p| p.reverse());
            }
        }
        p.map(|p| p.reverse())
    }
}

impl Ord for Entry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let p = self.stored.published.cmp(&other.stored.published);
        if p.is_eq() {
            self.stored
                .entry_id
                .0
                .cmp(&other.stored.entry_id.0)
                .reverse()
        } else {
            p.reverse()
        }
    }
}

#[derive(Clone)]
struct DisplayedEntries {
    unread: Vector<Entry>,
    read: Vector<Entry>,
}

impl Default for DisplayedEntries {
    fn default() -> Self {
        DisplayedEntries {
            unread: Vector::new(),
            read: Vector::new(),
        }
    }
}

enum EntriesAction {
    Replace(Vec<StoredEntry>),
    MarkPendingRead(EntryID, u64),
    MarkFinallyReadUnread(Vec<(EntryID, u64, i64)>),
    MarkPendingUnread(EntryID, u64),
    BackendResponse(Option<Vec<u8>>),
}

/// The entries processor coroutine, handles all the signals related to feed entries processing
#[derive(Clone)]
pub struct EntriesHandle {
    actor: CoroutineHandle<EntriesAction>,
    bridge: WorkerBridge<BackendWorker>,
}

impl EntriesHandle {
    pub fn handle_mark_read_unread(&self, entry_id: EntryID, published: u64, is_read: bool) {
        let handle = self.actor.clone();

        if is_read {
            handle.send(EntriesAction::MarkPendingRead(
                entry_id.clone(),
                published.clone(),
            ));
        } else {
            handle.send(EntriesAction::MarkPendingUnread(
                entry_id.clone(),
                published.clone(),
            ));
        }

        let event = Event::MarkReadUnread(vec![(entry_id.clone(), published, is_read)]);
        let data = serde_json::to_vec(&event).unwrap();
        self.bridge.send(WorkerRequest::BackendRequest(data));
    }

    pub fn handle_fetch_entries(&self) {
        let event = Event::AllEntries(AllEntries {
            unread_only: true,
            feed_id: None,
        });
        let data = serde_json::to_vec(&event).unwrap();
        self.bridge.send(WorkerRequest::BackendRequest(data));
    }

    pub fn handle_refresh(&self) {
        let event = Event::RefreshFeeds(RefreshFeeds { force: true });
        let data = serde_json::to_vec(&event).unwrap();
        self.bridge.send(WorkerRequest::BackendRequest(data));
        self.handle_fetch_entries();
    }
}

pub fn init_entries_actor(cx: &Scope) {
    let entries = use_atom_ref(&cx, ENTRIES).clone();
    let actor = use_coroutine(&cx, |mut rx: UnboundedReceiver<EntriesAction>| async move {
        while let Some(action) = rx.next().await {
            handle_action(action, entries.clone());
        }
    })
    .to_owned();

    let bridge = use_read(cx, WORKER_BRIDGE);

    let b_actor = actor.clone();
    let bridge = bridge.borrow().fork(Some(move |response| {
        // TODO move callback to a separate function
        if let WorkerResponse::BackendResponse(data) = response {
            // TODO: process data.is_none()
            // TODO: process response deserialize error
            // the callback means "when worker responds with the message"
            b_actor.send(EntriesAction::BackendResponse(data))
        } else {
            // TODO: send worker status change message
            todo!();
        }
    }));

    let handle = EntriesHandle {
        actor: actor.to_owned(),
        bridge,
    };
    use_context_provider(cx, move || handle);
}

fn find_entry<'a>(
    entries: &'a Vector<Entry>,
    id: &'a EntryID,
    published: &'a u64,
) -> Option<usize> {
    entries
        .iter()
        .position(|e| &e.stored.entry_id == id && &e.stored.published == published)
}

fn handle_action(action: EntriesAction, atom_entries: UseAtomRef<DisplayedEntries>) {
    // NOTDO: let mut entries = atom_entries.write();
    // we are recursively calling handle_action, so borrowing atom_entries for writing
    // will be double mutable borrowing
    match action {
        EntriesAction::Replace(mut new_entries) => {
            // sometimes entries are duplicated in feeds, so we need
            // to filter them out because UI requires uniques and we don't want to read dups
            // anyways
            let mut duplicates = HashSet::new();
            let mut entries = atom_entries.write();
            entries.unread.clear();
            while let Some(entry) = new_entries.pop() {
                let unread = entry.read_ts == 0;

                if !duplicates.contains(&entry.entry_id) {
                    duplicates.insert(entry.entry_id.clone());
                    entries.unread.insert_ord(Entry {
                        stored: entry,
                        unread,
                    });
                }
            }
        }

        EntriesAction::MarkPendingRead(id, published) => {
            let mut entries = atom_entries.write();
            if let Some(pos) = find_entry(&entries.unread, &id, &published) {
                let entry = entries.unread.remove(pos);
                match find_entry(&entries.read, &id, &published) {
                    // due to connection issues, after reloading unread entries,
                    // some entries may become duplicate in read list and break key uniqueness
                    // requirement in dioxus
                    None => {
                        //entries.read.insert_ord(entry);
                        entries.read.push_front(entry);
                    }
                    Some(pos) => {
                        // for such duplicate entries we need to replace them by the fetched ones
                        entries.read[pos] = entry;
                    }
                }
            } else {
                warn!("unread entry not found: {:?}", (id, published));
            }
            while entries.read.len() > MAX_READ_ENTRIES {
                entries.read.pop_back();
            }
        }

        EntriesAction::MarkFinallyReadUnread(mut marked) => {
            let mut entries = atom_entries.write();

            while let Some((entry_id, published, read_ts)) = marked.pop() {
                let list = if read_ts == 0 {
                    // message was about entry marked unread
                    &mut entries.unread
                } else {
                    &mut entries.read
                };
                if let Some(pos) = list
                    .iter()
                    .position(|e| e.stored.entry_id == entry_id && e.stored.published == published)
                {
                    list.get_mut(pos).map(|mut e| e.stored.read_ts = read_ts);
                } else {
                    warn!("entry not found: {:?}", (entry_id, published, read_ts));
                }
            }
        }

        EntriesAction::MarkPendingUnread(entry_id, published) => {
            let mut entries = atom_entries.write();
            if let Some(pos) = entries
                .read
                .iter()
                .position(|e| e.stored.entry_id == entry_id && e.stored.published == published)
            {
                let entry = entries.read.remove(pos);
                entries.unread.insert_ord(entry);
            } else {
                warn!("read entry not found: {:?}", (entry_id, published));
            }
        }

        EntriesAction::BackendResponse(data) => {
            if let Some(data) = data {
                warn!("worker response: {}", String::from_utf8_lossy(&data));
                let response = serde_json::from_slice(&data).expect("cannot deserialize response");

                warn!("worker response deserialized: {:?}", response);
                match response {
                    Response::AllEntries(entries) => {
                        handle_action(EntriesAction::Replace(entries), atom_entries.clone())
                    }
                    Response::MarkedRead(entries) => {
                        handle_action(
                            EntriesAction::MarkFinallyReadUnread(entries),
                            atom_entries.clone(),
                        );
                    }
                    _ => {
                        warn!("bad response from backend: {:?}", response);
                    }
                }
            } else {
                warn!("empty response from worker");
            }
        }
    }

    let entries = atom_entries.read();
    warn!(
        //trace!(
        "entries after action: {:?} {:?}",
        entries.read.len(),
        entries.unread.len()
    );
}

#[allow(non_snake_case)]
pub fn Entries(cx: Scope) -> Element {
    let entries = use_atom_ref(&cx, ENTRIES);
    let entries_handle = use_context::<EntriesHandle>(&cx).unwrap().to_owned();
    let entries: &DisplayedEntries = &entries.read();

    let entry_nodes = entries.unread.iter().map(|entry| {
        let link = entry.stored.link.clone().unwrap_or_default();
        let title = entry.stored.title.clone().unwrap_or_default();

        let html_description = parse_description(entry.stored.description.as_ref().unwrap_or(&String::new()));

        let id = entry.stored.entry_id.clone();
        let published = entry.stored.published.clone();
        let read = entry.stored.read_ts != 0;
        let key = id.0.clone();

       rsx!(
        p {
            key: "{key}",
            div {
                a {
                    target: "_blank",
                    href: "{link}",
                    rel: "noopener noreferrer",
                    "{title}"
                }
                div {
                    dangerous_inner_html: "{html_description}",
                }
                button {
                    disabled: "{read}",
                    onclick: move |_| entries_handle.read().handle_mark_read_unread(id.clone(), published, true),
                    "Read>>"
                }
            }
        }
        )
    });
    cx.render(rsx! {
       entry_nodes
    })
}

#[allow(non_snake_case)]
pub fn ReadEntries(cx: Scope) -> Element {
    let entries = use_atom_ref(&cx, ENTRIES);
    let entries_handle = use_context::<EntriesHandle>(&cx).unwrap().to_owned();
    let entries: &DisplayedEntries = &entries.read();
    let entry_nodes = entries.read.iter().map(|entry| {
        let link = entry.stored.link.clone().unwrap_or_default();
        let title = entry.stored.title.clone().unwrap_or_default();

        let id = entry.stored.entry_id.clone();
        let published = entry.stored.published.clone();
        let read = entry.stored.read_ts == 0;
        let key = id.0.clone();

        rsx!(
        p {
            key: "{key}",
            div {
                small {
                    button {
                        disabled: "{read}",
                        onclick: move |_| entries_handle.read().handle_mark_read_unread(id.clone(), published, false),
                        "<<"
                    }
                    a {
                        target: "_blank",
                        href: "{link}",
                        rel: "noopener noreferrer",
                        "{title}"
                    }
                }
            }
        }
        )
    });
    cx.render(rsx! {
       entry_nodes
    })
}

fn parse_description<'a>(description: &'a str) -> String {
    use quick_xml::events::Event;
    use quick_xml::Reader;
    // parsing trees in Rust is hard, so we'll replace it with filtering only

    let mut reader = Reader::from_str(description);
    let mut buf = Vec::new();
    let mut result = String::new();

    loop {
        match reader.read_event(&mut buf) {
            Ok(Event::Start(ref e)) => match e.name() {
                name @ b"a" => {
                    let mut after = String::new();
                    // for now only allow h single href attribute
                    for attr in e.attributes() {
                        if let Ok(attr) = attr {
                            if attr.key == b"href" {
                                after.push_str(" href=\"");
                                after.push_str(&String::from_utf8_lossy(&attr.value));
                                after.push('"');
                            }
                        }
                    }

                    push_tag(&mut result, "", name, &after);
                }
                name => {
                    push_tag(&mut result, "", name, "");
                }
            },
            Ok(Event::Empty(e)) => push_tag(&mut result, "", e.name(), "/"),
            Ok(Event::End(e)) => push_tag(&mut result, "", e.name(), ""),
            Ok(Event::Text(e)) => result.push_str(
                &e.unescape_and_decode(&reader)
                    .unwrap_or("BAD HTML".to_string()),
            ),
            Ok(Event::CData(text)) => {
                result.push_str("<![CDATA[");
                result.push_str(
                    &text
                        .unescaped()
                        .as_ref()
                        .map(|s| String::from_utf8_lossy(s))
                        .unwrap_or(std::borrow::Cow::Borrowed("BAD HTML")),
                );
                result.push_str("...]]>");
            }
            Err(_e) => {
                result =
                    String::from_utf8_lossy(&quick_xml::escape::escape(description.as_bytes()))
                        .to_string();
                break;
            }
            Ok(Event::Eof) => break,
            _ => (),
        }
    }

    result
}

fn push_tag(s: &mut String, before: &str, e: &[u8], after: &str) {
    let allowed = match e {
        b"a" | b"p" | b"i" | b"b" | b"u" | b"hr" => true,
        _ => false,
    };
    if allowed {
        s.push_str("<");
    } else {
        s.push_str("&lt;");
    }
    s.push_str(before);
    s.push_str(&String::from_utf8_lossy(e));
    s.push_str(after);
    if allowed {
        s.push_str(">");
    } else {
        s.push_str("&gt;");
    }
}
