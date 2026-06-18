/* ParentTable */
create table ParentTable
(
   ID integer auto_increment not null,
   Name varchar(100) not null,
   Extra varchar(200),
   Gender enum('MALE', 'FEMALE')
);

create index ix_ParentTable1 on ParentTable (Extra, Name);
create index ix_ParentTable2 on ParentTable (ID, Name, Extra);

insert into ParentTable (Name,Extra,Gender) values ('AAA','Extra AAA','M');
insert into ParentTable (Name,Extra,Gender) values ('BBB','Extra BBB','F');

/* ChildTable */
create table ChildTable
(
   ID integer auto_increment not null,
   ParentID integer not null,
   Name varchar(100) not null
);

/* ColumnTesterTable */
create table ColumnTesterTable
(
   sequence integer auto_increment not null,
   longsequence bigint auto_increment,
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
   varchar varchar(10),
   varcharWithCheck varchar(6),
   enum enum('ONE', 'TWO'),
   text mediumtext,
   binary mediumblob,
   uuid char(36),
   json mediumtext,
   constraint ck_columntes_int_D0ABB2EE0687D3EA check(int >= 1 and int <= 500),
   constraint ck_columntes_varcharwi_A492D40E82ACBF3A varcharWithCheck = 'ABC123'
);

/* Property */
create table Property
(
   ID integer auto_increment not null,
   Name varchar(50) not null,
   ShortName varchar(25) not null,
   Code varchar(25) not null,
   AltCode varchar(25) not null,
   NumberRooms smallint not null,
   RegionID integer,
   constraint ck_property_numberroo_86E29E032F5EC48B check(NumberRooms >= 0 and NumberRooms <= 20000)
);

/* Region */
create table Region
(
   ID integer auto_increment not null,
   Name varchar(50) not null,
   ShortName varchar(25) not null,
   Code varchar(25) not null,
   ExcludeFromCorpReports boolean not null constraint ExcludeFromCorpReports default false
);

/* KBI */
create table KBI
(
   ID integer auto_increment not null,
   PropertyID integer not null,
   Name varchar(50) not null,
   Code varchar(25) not null,
   ShowInModule enum('ALL', 'BUDGET', 'LABOR') not null,
   MasterKBICodeID integer,
   UnitID integer
);

create index ix_KBI1 on KBI (MasterKBICodeID);

/* MasterKBICode */
create table MasterKBICode
(
   ID integer auto_increment not null,
   Code varchar(25) not null,
   Description varchar(50) not null,
   ShowOnDashboard boolean not null constraint ShowOnDashboard default false,
   SortOrder integer,
   GroupingFreeForm varchar(50)
);

/* test.Unit */
create table test.Unit
(
   ID integer auto_increment not null,
   PropertyID integer not null,
   Name varchar(50) not null,
   SingularName varchar(50) not null,
   Symbol varchar(5) not null,
   Comment varchar(255)
);

/* relations */
alter table ChildTable add constraint fk_childtable1 foreign key (ParentID) references ParentTable(ID) on delete cascade;
alter table Property add constraint fk_property1 foreign key (RegionID) references Region(ID) on delete setnull;
alter table KBI add constraint fk_kbi1 foreign key (PropertyID) references Property(ID) on delete cascade;
alter table KBI add constraint fk_kbi2 foreign key (UnitID) references test.Unit(ID) on delete setnull;
alter table KBI add constraint fk_kbi3 foreign key (MasterKBICodeID) references MasterKBICode(ID) on delete setnull;
alter table test.Unit add constraint fk_unit1 foreign key (PropertyID) references Property(ID) on delete cascade;

