create schema if not exists test;

create or replace function generate_uuid() returns uuid language plpgsql volatile parallel unsafe as $$
declare
   -- The current UNIX timestamp in milliseconds
   unix_time_ms CONSTANT bigint NOT NULL DEFAULT (extract(epoch FROM clock_timestamp()) * 1000)::bigint;

   -- The buffer used to create the UUID: the low 6 bytes (48 bits) of the timestamp, followed by 10 random bytes
   buffer bytea not null default substring(int8send(unix_time_ms) from 3) || gen_random_bytes(10);
begin
   -- Set the version nibble of byte 6 to 0111 (UUID v7), keeping the last 4 bits unchanged
   buffer = set_byte(buffer, 6, (get_byte(buffer, 6) & 15) | 112);

   -- Set the top 2 bits of byte 8 to 10 (the UUID variant specified in RFC 4122), keeping the last 6 bits unchanged
   buffer = set_byte(buffer, 8, (get_byte(buffer, 8) & 63) | 128);

   return encode(buffer, 'hex')::uuid;
end
$$;

do $$
begin
   if (select usesuper from pg_user where usename = CURRENT_USER) then
      create extension if not exists "pgcrypto";
      create extension if not exists "citext";
      create extension if not exists "btree_gist";
   else
      raise notice 'Could not create extensions, user % does not have permission.', current_user;
   end if;
end;
$$;

drop type if exists gender_type cascade;
create type gender_type as enum ('M','F');

drop type if exists show_in_module_type cascade;
create type show_in_module_type as enum ('A','B','L');

drop type if exists test_enum_type cascade;
create type test_enum_type as enum ('1','2');

other top sql for pgsql 1;

other top sql for pgsql 2;

/* public.Assignment */
drop table if exists public.Assignment cascade;

create table public.Assignment
(
   ID serial not null,
   PropertyID integer not null,
   ParentAssignmentID integer,
   Name text not null,
   constraint pk_assignment primary key (ID),
   constraint ak_assignment1 unique (ID,PropertyID)
);

/* public.ChildTable */
drop table if exists public.ChildTable cascade;

create table public.ChildTable
(
   ID serial not null,
   ParentID integer not null,
   Name text not null,
   constraint pk_childtable primary key (ID),
   constraint ak_childtable1 unique (ParentID,Name)
);

/* public.ColumnTesterTable */
drop table if exists public.ColumnTesterTable cascade;

create table public.ColumnTesterTable
(
   sequence serial not null,
   byte smallint,
   short smallint,
   int integer,
   long bigint,
   float real,
   double double precision,
   decimal decimal(19,4),
   boolean boolean,
   date date,
   datetime timestamp,
   time time,
   timestamp timestamp,
   char char(1) default default 'A',
   varchar text,
   varcharWithCheck text,
   enum test_enum_type,
   text text,
   binary bytea,
   uuid uuid,
   json jsonb,
   constraint ck_columntes_int_B3963409 check(int >= 1 and int <= 500),
   constraint ck_columntes_varcharwi_353F3BCB check(varcharWithCheck = 'ABC123')
);

/* public.KBI */
drop table if exists public.KBI cascade;

create table public.KBI
(
   ID serial not null,
   PropertyID integer not null,
   Name text not null,
   Code text not null,
   ShowInModule show_in_module_type not null,
   MasterKBICodeID integer,
   UnitID integer,
   constraint pk_kbi primary key (ID),
   constraint ak_kbi1 unique (PropertyID,Name),
   constraint ak_kbi2 unique (PropertyID,Code)
);

create index ix_kbi1 on public.KBI (MasterKBICodeID);

/* public.LongSequenceTesterTable */
drop table if exists public.LongSequenceTesterTable cascade;

create table public.LongSequenceTesterTable
(
   longsequence bigserial not null
);

/* public.MasterKBICode */
drop table if exists public.MasterKBICode cascade;

create table public.MasterKBICode
(
   ID serial not null,
   Code text not null,
   Description text not null,
   ShowOnDashboard boolean not null,
   SortOrder integer,
   GroupingFreeForm text,
   constraint pk_masterkbicode primary key (ID),
   constraint ak_masterkbicode1 unique (Code)
);

/* public.ParentTable */
drop table if exists public.ParentTable cascade;

create table public.ParentTable
(
   ID serial not null,
   Name text not null,
   Extra text,
   Gender gender_type,
   constraint pk_parenttable primary key (ID),
   constraint ak_parenttable1 unique (Name,Extra)
);

create index ix_parenttable1 on public.ParentTable (Extra, Name);
create index ix_parenttable2 on public.ParentTable (ID, Name, Extra);

insert into ParentTable (Name,Extra,Gender) values ('AAA','Extra AAA','M');
insert into ParentTable (Name,Extra,Gender) values ('BBB','Extra BBB','F');
insert into ParentTable (Name,Extra,Gender) values ('PGSQL','Extra PGSQL','M');

/* public.Property */
drop table if exists public.Property cascade;

create table public.Property
(
   ID serial not null,
   Name text not null,
   ShortName text not null,
   Code text not null,
   AltCode text not null,
   NumberRooms smallint not null,
   RegionID integer,
   constraint pk_property primary key (ID),
   constraint ak_property1 unique (Name),
   constraint ak_property2 unique (Code),
   constraint ak_property3 unique (AltCode),
   constraint ck_property_numberroo_90DF89E5 check(NumberRooms >= 0 and NumberRooms <= 20000)
);

/* public.Region */
drop table if exists public.Region cascade;

create table public.Region
(
   ID serial not null,
   Name text not null,
   ShortName text not null,
   Code text not null,
   ExcludeFromCorpReports boolean not null,
   constraint pk_region primary key (ID),
   constraint ak_region1 unique (Name),
   constraint ak_region2 unique (Code)
);

/* test.Unit */
drop table if exists test.Unit cascade;

create table test.Unit
(
   ID serial not null,
   PropertyID integer not null,
   Name text not null,
   SingularName text not null,
   Symbol text not null,
   Comment text,
   constraint pk_unit primary key (ID),
   constraint ak_unit1 unique (PropertyID,Name),
   constraint ak_unit2 unique (PropertyID,SingularName)
);

/* relations */
alter table public.Assignment add constraint fk_assignment1 foreign key (PropertyID) references public.Property(ID) on delete cascade;
alter table public.Assignment add constraint fk_assignment2 foreign key (ParentAssignmentID, PropertyID) references public.Assignment(ID, PropertyID) on delete cascade;
alter table public.ChildTable add constraint fk_childtable1 foreign key (ParentID) references public.ParentTable(ID) on delete cascade;
alter table public.KBI add constraint fk_kbi1 foreign key (PropertyID) references public.Property(ID) on delete cascade;
alter table public.KBI add constraint fk_kbi2 foreign key (UnitID) references test.Unit(ID) on delete set null;
alter table public.KBI add constraint fk_kbi3 foreign key (MasterKBICodeID) references public.MasterKBICode(ID) on delete set null;
alter table public.Property add constraint fk_property1 foreign key (RegionID) references public.Region(ID) on delete set null;
alter table test.Unit add constraint fk_unit1 foreign key (PropertyID) references public.Property(ID) on delete cascade;

/* public.parenttable_delete */
create or replace function public.parenttable_delete() returns trigger as $BODY$
begin
delete from pgsql
   return null;
end;
$BODY$ language plpgsql;

drop trigger if exists parenttable_delete on public.ParentTable cascade;
create trigger parenttable_delete after delete on public.ParentTable
   for each row execute procedure public.parenttable_delete();

/* public.parenttable_update */
create or replace function public.parenttable_update() returns trigger as $BODY$
begin
update pgsql
   return new;
end;
$BODY$ language plpgsql;

drop trigger if exists parenttable_update on public.ParentTable cascade;
create trigger parenttable_update after insert or update on public.ParentTable
   for each row execute procedure public.parenttable_update();

sql-function-1;

sql-function-2;

custom function sql for pgsql 1;

custom function sql for pgsql 2;

/* public.TestView1 */
create or replace view public.TestView1 as
   select * from ParentTable;

/* public.TestView2 */
create or replace view public.TestView2 as
   select * from pgsql;

/* test.TestView1 */
create or replace view test.TestView1 as
   select * from ParentTable;

sql-procedure-1;

sql-procedure-2;

custom procedure sql for pgsql 1;

custom procedure sql for pgsql 2;

custom procedure sql for mssql 2;

other bottom sql for pgsql 1;

other bottom sql for pgsql 2;

