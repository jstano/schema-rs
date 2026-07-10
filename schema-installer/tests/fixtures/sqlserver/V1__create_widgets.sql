create table widgets (
    id int identity(1,1) primary key,
    name nvarchar(max) not null,
    sku nvarchar(50) not null unique,
    created_at datetime not null default getutcdate()
);
GO

create index idx_widgets_sku on widgets (sku);
GO
