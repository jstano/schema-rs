create table widgets (
    id integer primary key autoincrement,
    name text not null,
    sku varchar(50) not null unique,
    created_at timestamp not null default current_timestamp
);

create index idx_widgets_sku on widgets (sku);
