//! Collision behaviour for `TagAllocator`.
//!
//! Duplicate tags would either make the core reject the config or silently
//! merge two outbounds — both are silent data loss, so the allocator must
//! always produce something unique. Everything here is pure logic.

use std::collections::HashSet;

use zeytun_lib::core::compiler::tag_allocator::{allocate_named_tag, TagAllocator};
use zeytun_lib::core::models::{policy::ProxyPolicy, proxy::ProxyServer};

// ProxyServer flattens its Protocol, so the variant's fields sit at the top
// level. Only tag/name matter to the allocator; chain needs no extra fields.
fn server(tag: &str, name: &str) -> ProxyServer {
    serde_json::from_value(serde_json::json!({
        "tag": tag,
        "name": name,
        "address": "1.2.3.4",
        "port": 443,
        "type": "chain",
        "proxies": [],
    }))
    .expect("valid ProxyServer fixture")
}

fn policy(tag: &str, name: &str) -> ProxyPolicy {
    serde_json::from_value(serde_json::json!({
        "tag": tag,
        "name": name,
        "outbounds": [],
        "type": "manual",
    }))
    .expect("valid ProxyPolicy fixture")
}

#[test]
fn fallback_collides_when_the_db_tag_repeats() {
    // This is the boundary worth stating: two proxies with an empty db tag
    // both key on "" in proxy_server_tags, so both calls resolve to the same
    // outbound tag. Real callers always have a unique db tag, so the fallback
    // never collides in practice — and when the db tag is set, it is used
    // verbatim, which is why distinct db tags cannot collide either.
    let mut alloc = TagAllocator::new();
    let a = alloc.allocate_proxy_server_tag(&server("", "My Node"));
    let b = alloc.allocate_proxy_server_tag(&server("", "My Node"));
    assert_eq!(a, b);
    assert_eq!(a, "proxy-my-node");

    let with_tag = alloc.allocate_proxy_server_tag(&server("db-1", "node"));
    assert_eq!(with_tag, "db-1", "a set db tag always wins");
}

#[test]
fn falls_back_to_name_when_tag_is_blank() {
    let mut alloc = TagAllocator::new();
    let tag = alloc.allocate_proxy_server_tag(&server("", "My Node"));
    assert_eq!(tag, "proxy-my-node");
}

#[test]
fn falls_back_when_both_blank() {
    let mut alloc = TagAllocator::new();
    let tag = alloc.allocate_proxy_server_tag(&server("", "   "));
    assert!(!tag.is_empty(), "blank proxy still needs a tag");
}

#[test]
fn same_input_is_stable() {
    let mut a = TagAllocator::new();
    let mut b = TagAllocator::new();
    for i in 0..10 {
        let s = server(&format!("p{i}"), &format!("Node {i}"));
        assert_eq!(
            a.allocate_proxy_server_tag(&s),
            b.allocate_proxy_server_tag(&s)
        );
    }
}

#[test]
fn duplicate_allocation_returns_the_same_tag() {
    let mut alloc = TagAllocator::new();
    let first = alloc.allocate_proxy_server_tag(&server("p1", "A"));
    let second = alloc.allocate_proxy_server_tag(&server("p1", "A"));
    assert_eq!(first, second, "reallocating the same id must be idempotent");
}

#[test]
fn proxy_and_policy_namespaces_do_not_collide() {
    let mut alloc = TagAllocator::new();
    let proxy_tag = alloc.allocate_proxy_server_tag(&server("shared", "A"));
    let policy_tag = alloc.allocate_policy_tag(&policy("shared", "A"));
    assert_ne!(
        proxy_tag, policy_tag,
        "the same preferred tag in both namespaces must stay distinct"
    );
}

#[test]
fn built_in_tags_are_reserved() {
    let mut alloc = TagAllocator::new();
    for reserved in &["direct", "block", "dns-out"] {
        assert!(alloc.is_used(reserved));
        let tag = alloc.allocate_proxy_server_tag(&server(reserved, "x"));
        assert_ne!(&tag, reserved, "cannot reuse a built-in tag");
    }
}

#[test]
fn unicode_and_whitespace_are_sanitized() {
    // Non-ASCII becomes a single dash run, so Ünïcødé -> n-c-d.
    assert_eq!(
        allocate_named_tag("p", "  Ünïcødé  ", &mut HashSet::new()),
        "p-n-c-d"
    );
    // Leading/trailing separators are trimmed, so this never yields "-p".
    assert_eq!(allocate_named_tag("p", "!!!", &mut HashSet::new()), "p");
    assert_eq!(allocate_named_tag("p", "", &mut HashSet::new()), "p");
}

#[test]
fn sanitize_collapses_runs_of_separators() {
    let mut used = HashSet::new();
    assert_eq!(allocate_named_tag("p", "a b_c", &mut used), "p-a-b_c");
}

#[test]
fn allocate_named_tag_collides_safely() {
    let mut used = HashSet::new();
    assert_eq!(allocate_named_tag("p", "node", &mut used), "p-node");
    assert_eq!(allocate_named_tag("p", "node", &mut used), "p-node-2");
    assert_eq!(allocate_named_tag("p", "node", &mut used), "p-node-3");
}
