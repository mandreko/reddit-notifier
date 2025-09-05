-- SQLx reversible migration (up)
CREATE TABLE IF NOT EXISTS subscriptions (
                                             id integer GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
                                             subreddit text NOT NULL,
                                             created_at timestamp without time zone NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS endpoints (
                                         id integer GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
                                         kind text NOT NULL CHECK (kind = ANY (ARRAY['discord'::text, 'pushover'::text])),
    config_json text NOT NULL,
    active boolean NOT NULL DEFAULT true
    );


CREATE TABLE IF NOT EXISTS subscription_endpoints (
                                                      subscription_id integer REFERENCES subscriptions(id) ON DELETE CASCADE,
    endpoint_id integer REFERENCES endpoints(id) ON DELETE CASCADE,
    PRIMARY KEY (subscription_id, endpoint_id)
    );


CREATE TABLE IF NOT EXISTS notified_posts (
                                              id integer GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
                                              subreddit text NOT NULL,
                                              post_id text NOT NULL,
                                              first_seen_at timestamp without time zone DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX notified_posts_unique_subreddit_post_id ON notified_posts(subreddit text_ops,post_id text_ops);
