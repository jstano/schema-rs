create or replace function generate_uuid() returns uuid language plpgsql parallel safe as $$
declare
   -- The current UNIX timestamp in milliseconds
   unix_time_ms CONSTANT bytea NOT NULL DEFAULT substring(int8send((extract(epoch FROM clock_timestamp()) * 1000)::bigint) from 3);

   -- The buffer used to create the UUID, starting with the UNIX timestamp and followed by random bytes
   buffer bytea not null default unix_time_ms || gen_random_bytes(10);
begin
   -- Set most significant 4 bits of 7th byte to 7 (for UUID v7), keeping the last 4 bits unchanged
   buffer = set_byte(buffer, 6, (b'0111' || get_byte(buffer, 6)::bit(4))::bit(8)::int);

   -- Set most significant 2 bits of 9th byte to 2 (the UUID variant specified in RFC 4122), keeping the last 6 bits unchanged
   buffer = set_byte(buffer, 8, (b'10' || get_byte(buffer, 8)::bit(6))::bit(8)::int);

   return encode(buffer, 'hex');
end
$$;

do $createextensions$
begin
   if (select usesuper from pg_user where usename = CURRENT_USER) then
      create extension if not exists "uuid-ossp";
      create extension if not exists "citext";
      create extension if not exists "btree_gist";
   else
      raise notice 'User % is not a superuser, could not create uuid-ossp or citext extensions.', current_user;
   end if;
end;
$createextensions$;

drop type if exists show_in_module_type cascade;
create type show_in_module_type as enum ('A', 'B', 'L');
drop type if exists gender_type cascade;
create type gender_type as enum ('M', 'F');
drop type if exists test_enum_type cascade;
create type test_enum_type as enum ('1', '2');


/* ParentTable */
create table ParentTable
(
   ID serial not null,
   Name text not null,
   Extra text,
   Gender gender_type
);

create index ix_ParentTable1 on ParentTable (Extra, Name);
create index ix_ParentTable2 on ParentTable (ID, Name, Extra);

insert into ParentTable (Name,Extra,Gender) values ('AAA','Extra AAA','M');
insert into ParentTable (Name,Extra,Gender) values ('BBB','Extra BBB','F');
insert into ParentTable (Name,Extra,Gender) values ('PGSQL','Extra PGSQL','M');

/* ChildTable */
create table ChildTable
(
   ID serial not null,
   ParentID integer not null,
   Name text not null
);

/* ColumnTesterTable */
create table ColumnTesterTable
(
   sequence serial not null,
   longsequence bigserial,
   byte tinyint,
   short smallint,
   int integer,
   long bigint,
   float real,
   double double precision,
   decimal decimal(19,4),
   boolean boolean constraint boolean default false,
   date date,
   datetime timestamp,
   time time,
   timestamp timestamp,
   char char(1) constraint char default default 'A',
   varchar text,
   varcharWithCheck text,
   enum test_enum_type,
   text text,
   binary bytea,
   uuid uuid,
   json jsonb,
   constraint ck_columntes_int_D0ABB2EE0687D3EA check(int >= 1 and int <= 500),
   constraint ck_columntes_varcharwi_A492D40E82ACBF3A varcharWithCheck = 'ABC123'
);

/* Property */
create table Property
(
   ID serial not null,
   Name text not null,
   ShortName text not null,
   Code text not null,
   AltCode text not null,
   NumberRooms smallint not null,
   RegionID integer,
   constraint ck_property_numberroo_86E29E032F5EC48B check(NumberRooms >= 0 and NumberRooms <= 20000)
);

/* Region */
create table Region
(
   ID serial not null,
   Name text not null,
   ShortName text not null,
   Code text not null,
   ExcludeFromCorpReports boolean not null constraint ExcludeFromCorpReports default false
);

/* KBI */
create table KBI
(
   ID serial not null,
   PropertyID integer not null,
   Name text not null,
   Code text not null,
   ShowInModule show_in_module_type not null,
   MasterKBICodeID integer,
   UnitID integer
);

create index ix_KBI1 on KBI (MasterKBICodeID);

/* MasterKBICode */
create table MasterKBICode
(
   ID serial not null,
   Code text not null,
   Description text not null,
   ShowOnDashboard boolean not null constraint ShowOnDashboard default false,
   SortOrder integer,
   GroupingFreeForm text
);

/* test.Unit */
create table test.Unit
(
   ID serial not null,
   PropertyID integer not null,
   Name text not null,
   SingularName text not null,
   Symbol text not null,
   Comment text
);

/* relations */
alter table ChildTable add constraint fk_childtable1 foreign key (ParentID) references ParentTable(ID) on delete cascade;
alter table Property add constraint fk_property1 foreign key (RegionID) references Region(ID) on delete setnull;
alter table KBI add constraint fk_kbi1 foreign key (PropertyID) references Property(ID) on delete cascade;
alter table KBI add constraint fk_kbi2 foreign key (UnitID) references test.Unit(ID) on delete setnull;
alter table KBI add constraint fk_kbi3 foreign key (MasterKBICodeID) references MasterKBICode(ID) on delete setnull;
alter table test.Unit add constraint fk_unit1 foreign key (PropertyID) references Property(ID) on delete cascade;

sql-function-1;

sql-function-2;

custom function sql for pgsql 1;

custom function sql for pgsql 2;

sql-procedure-1;

sql-procedure-2;

custom procedure sql for pgsql 1;

custom procedure sql for pgsql 2;

custom procedure sql for mssql 2;

