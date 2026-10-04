#[derive(Debug, Clone)]
pub enum CompileWarning {
    ProxyServerIgnored {
        proxy_server_tag: String,
        name: String,
        reason: String,
    },
    ProxyServerNormalized {
        proxy_server_tag: String,
        name: String,
        reason: String,
    },
    PolicyIgnored {
        proxy_policy_tag: String,
        name: String,
        reason: String,
    },
    PolicyMemberIgnored {
        proxy_policy_tag: String,
        member: String,
        reason: String,
    },
    FinalPolicyChanged {
        from: String,
        to: String,
        reason: String,
    },
    RuleIgnored {
        rule_index: usize,
        reason: String,
    },
}
