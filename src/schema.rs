table! {
    logins (id) {
        id -> Nullable<Integer>,
        name -> Text,
        email -> Text,
        password_hash -> Text,
    }
}

table! {
    papers (id) {
        id -> Nullable<Integer>,
        title -> Text,
        url -> Text,
        venue -> Nullable<Text>,
    }
}

table! {
    votes (id) {
        id -> Nullable<Integer>,
        paper_id -> Integer,
        user_id -> Integer,
    }
}

joinable!(votes -> logins (user_id));
joinable!(votes -> papers (paper_id));

allow_tables_to_appear_in_same_query!(
    logins,
    papers,
    votes,
);
