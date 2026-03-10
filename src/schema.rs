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
        publication_year -> Nullable<Integer>,
        user_id -> Integer,
        vote_count -> Integer,
        readed -> Integer,
        pdf_file -> Nullable<Text>,
    }
}

table! {
    votes (id) {
        id -> Nullable<Integer>,
        paper_id -> Integer,
        user_id -> Integer,
    }
}

joinable!(papers -> logins (user_id));
joinable!(votes -> logins (user_id));
joinable!(votes -> papers (paper_id));

allow_tables_to_appear_in_same_query!(logins, papers, votes,);
