drop table if exists ChildTable;
/* ChildTable */
drop table if exists ColumnTesterTable;
/* ColumnTesterTable */
drop table if exists KBI;
/* KBI */
drop table if exists LongSequenceTesterTable;
/* LongSequenceTesterTable */
drop table if exists ParentTable;
/* ParentTable */
drop table if exists test_Unit;
/* test_Unit */
drop table if exists MasterKBICode;
/* MasterKBICode */
drop table if exists Property;
/* Property */
drop table if exists Region;
/* Region */
create table ChildTable
(
   ID integer primary key autoincrement,
   ParentID integer not null,
   Name varchar(100) not null,
   constraint ak_childtable1 unique (ParentID,Name),
   constraint fk_childtable1 foreign key (ParentID) references ParentTable(ID) on delete cascade
);

create table ColumnTesterTable
(
   sequence integer not null,
   byte tinyint,
   short smallint,
   int integer,
   long bigint,
   float real,
   double double precision,
   decimal decimal(19,4),
   boolean boolean,
   date text,
   datetime text,
   time text,
   timestamp text,
   char char(1) constraint char default default 'A',
   varchar varchar(10),
   varcharWithCheck varchar(6),
   enum char(1),
   text text,
   binary blob,
   uuid text,
   json text,
   constraint ck_columntes_int_B3963409 check(int >= 1 and int <= 500),
   constraint ck_columntes_varcharwi_353F3BCB check(varcharWithCheck = 'ABC123'),
   constraint ck_columntes_enum_BF2E7C27 check(enum in ('1','2'))
);

create table KBI
(
   ID integer primary key autoincrement,
   PropertyID integer not null,
   Name varchar(50) not null,
   Code varchar(25) not null,
   ShowInModule char(1) not null,
   MasterKBICodeID integer,
   UnitID integer,
   constraint ak_kbi1 unique (PropertyID,Name),
   constraint ak_kbi2 unique (PropertyID,Code),
   constraint ck_kbi_showinmod_B47F96FB check(ShowInModule in ('A','B','L')),
   constraint fk_kbi1 foreign key (PropertyID) references Property(ID) on delete cascade,
   constraint fk_kbi2 foreign key (UnitID) references test_Unit(ID) on delete set null,
   constraint fk_kbi3 foreign key (MasterKBICodeID) references MasterKBICode(ID) on delete set null
);

create index ix_kbi1 on KBI (MasterKBICodeID);

create table LongSequenceTesterTable
(
   longsequence integer not null
);

create table MasterKBICode
(
   ID integer primary key autoincrement,
   Code varchar(25) not null,
   Description varchar(50) not null,
   ShowOnDashboard boolean not null,
   SortOrder integer,
   GroupingFreeForm varchar(50),
   constraint ak_masterkbicode1 unique (Code)
);

create table ParentTable
(
   ID integer primary key autoincrement,
   Name varchar(100) not null,
   Extra varchar(200),
   Gender char(1),
   constraint ak_parenttable1 unique (Name,Extra),
   constraint ck_parenttab_gender_E250C9FC check(Gender in ('M','F'))
);

create index ix_parenttable1 on ParentTable (Extra, Name);
create index ix_parenttable2 on ParentTable (ID, Name, Extra);

insert into ParentTable (Name,Extra,Gender) values ('AAA','Extra AAA','M');
insert into ParentTable (Name,Extra,Gender) values ('BBB','Extra BBB','F');

create table Property
(
   ID integer primary key autoincrement,
   Name varchar(50) not null,
   ShortName varchar(25) not null,
   Code varchar(25) not null,
   AltCode varchar(25) not null,
   NumberRooms smallint not null,
   RegionID integer,
   constraint ak_property1 unique (Name),
   constraint ak_property2 unique (Code),
   constraint ak_property3 unique (AltCode),
   constraint ck_property_numberroo_90DF89E5 check(NumberRooms >= 0 and NumberRooms <= 20000),
   constraint fk_property1 foreign key (RegionID) references Region(ID) on delete set null
);

create table Region
(
   ID integer primary key autoincrement,
   Name varchar(50) not null,
   ShortName varchar(25) not null,
   Code varchar(25) not null,
   ExcludeFromCorpReports boolean not null,
   constraint ak_region1 unique (Name),
   constraint ak_region2 unique (Code)
);

create table test_Unit
(
   ID integer primary key autoincrement,
   PropertyID integer not null,
   Name varchar(50) not null,
   SingularName varchar(50) not null,
   Symbol varchar(5) not null,
   Comment varchar(255),
   constraint ak_unit1 unique (PropertyID,Name),
   constraint ak_unit2 unique (PropertyID,SingularName),
   constraint fk_unit1 foreign key (PropertyID) references Property(ID) on delete cascade
);

/* TestView1 */
drop view if exists TestView1;
create view TestView1 as
   select * from ParentTable;

/* test_TestView1 */
drop view if exists test_TestView1;
create view test_TestView1 as
   select * from ParentTable;

