create table widgets (
    id bigserial primary key,
    name text not null,
    sku varchar(50) not null unique,
    created_at timestamp not null default current_timestamp
);

create index idx_widgets_sku on widgets (sku);
