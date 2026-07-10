other top sql for mssql 1;

other top sql for mssql 2;

/* ParentTable */
create table ParentTable
(
   ID integer identity(1,1) not null,
   Name nvarchar(100) not null,
   Extra nvarchar(200),
   Gender char(1),
   constraint pk_parenttable primary key (ID),
   constraint ak_parenttable1 unique (Name,Extra),
   constraint ck_parenttab_gender_1CE737348B58A5C6 check(Gender in ('M', 'F'))
);

create index ix_ParentTable1 on ParentTable (Extra, Name);
create index ix_ParentTable2 on ParentTable (ID, Name, Extra);

insert into ParentTable (Name,Extra,Gender) values ('AAA','Extra AAA','M');
insert into ParentTable (Name,Extra,Gender) values ('BBB','Extra BBB','F');
insert into ParentTable (Name,Extra,Gender) values ('MSSQL','Extra MSSQL','F');

/* ChildTable */
create table ChildTable
(
   ID integer identity(1,1) not null,
   ParentID integer not null,
   Name nvarchar(100) not null,
   constraint pk_childtable primary key (ID),
   constraint ak_childtable1 unique (ParentID,Name)
);

/* ColumnTesterTable */
create table ColumnTesterTable
(
   sequence integer identity(1,1) not null,
   longsequence bigint identity(1,1),
   byte tinyint,
   short smallint,
   int integer,
   long bigint,
   float real,
   double double precision,
   decimal decimal(19,4),
   boolean bit constraint boolean default false,
   date datetime,
   datetime datetime,
   time datetime,
   timestamp datetime,
   char nchar(1) constraint char default default 'A',
   varchar nvarchar(10),
   varcharWithCheck nvarchar(6),
   enum char(1),
   text nvarchar(max),
   binary varbinary(max),
   uuid uniqueidentifier,
   json json,
   constraint ck_columntes_int_D0ABB2EE0687D3EA check(int >= 1 and int <= 500),
   constraint ck_columntes_varcharwi_A492D40E82ACBF3A varcharWithCheck = 'ABC123',
   constraint ck_columntes_enum_338DAC892CFFD8A0 check(enum in ('1', '2'))
);

alter table ColumnTesterTable set (lock_escalation = DISABLE);

/* Property */
create table Property
(
   ID integer identity(1,1) not null,
   Name nvarchar(50) not null,
   ShortName nvarchar(25) not null,
   Code nvarchar(25) not null,
   AltCode nvarchar(25) not null,
   NumberRooms smallint not null,
   RegionID integer,
   constraint pk_property primary key (ID),
   constraint ak_property1 unique (Name),
   constraint ak_property2 unique (Code),
   constraint ak_property3 unique (AltCode),
   constraint ck_property_numberroo_86E29E032F5EC48B check(NumberRooms >= 0 and NumberRooms <= 20000)
);

/* Region */
create table Region
(
   ID integer identity(1,1) not null,
   Name nvarchar(50) not null,
   ShortName nvarchar(25) not null,
   Code nvarchar(25) not null,
   ExcludeFromCorpReports bit not null constraint ExcludeFromCorpReports default false,
   constraint pk_region primary key (ID),
   constraint ak_region1 unique (Name),
   constraint ak_region2 unique (Code)
);

/* KBI */
create table KBI
(
   ID integer identity(1,1) not null,
   PropertyID integer not null,
   Name nvarchar(50) not null,
   Code nvarchar(25) not null,
   ShowInModule char(1) not null,
   MasterKBICodeID integer,
   UnitID integer,
   constraint pk_kbi primary key (ID),
   constraint ak_kbi1 unique (PropertyID,Name),
   constraint ak_kbi2 unique (PropertyID,Code),
   constraint ck_kbi_showinmod_B69FD4D2074BE1A8 check(ShowInModule in ('A', 'B', 'L'))
);

create index ix_KBI1 on KBI (MasterKBICodeID);

/* MasterKBICode */
create table MasterKBICode
(
   ID integer identity(1,1) not null,
   Code nvarchar(25) not null,
   Description nvarchar(50) not null,
   ShowOnDashboard bit not null constraint ShowOnDashboard default false,
   SortOrder integer,
   GroupingFreeForm nvarchar(50),
   constraint pk_masterkbicode primary key (ID),
   constraint ak_masterkbicode1 unique (Code)
);

/* test.Unit */
create table test.Unit
(
   ID integer identity(1,1) not null,
   PropertyID integer not null,
   Name nvarchar(50) not null,
   SingularName nvarchar(50) not null,
   Symbol nvarchar(5) not null,
   Comment nvarchar(255),
   constraint pk_unit primary key (ID),
   constraint ak_unit1 unique (PropertyID,Name),
   constraint ak_unit2 unique (PropertyID,SingularName)
);

/* relations */
alter table ChildTable add constraint fk_childtable1 foreign key (ParentID) references ParentTable(ID) on delete cascade;
alter table Property add constraint fk_property1 foreign key (RegionID) references Region(ID) on delete set null;
alter table KBI add constraint fk_kbi1 foreign key (PropertyID) references Property(ID) on delete cascade;
alter table KBI add constraint fk_kbi2 foreign key (UnitID) references test.Unit(ID) on delete set null;
alter table KBI add constraint fk_kbi3 foreign key (MasterKBICodeID) references MasterKBICode(ID) on delete set null;
alter table test.Unit add constraint fk_unit1 foreign key (PropertyID) references Property(ID) on delete cascade;

/* parenttable_delete */
if exists (select name from dbo.sysobjects where name = 'parenttable_delete' and type = 'TR')
   drop trigger parenttable_delete;

create trigger parenttable_delete on ParentTable for delete as
if (select count(*) from deleted) > 0
BEGIN
delete from mssql
END;

/* parenttable_update */
if exists (select name from dbo.sysobjects where name = 'parenttable_update' and type = 'TR')
   drop trigger parenttable_update;

create trigger parenttable_update on ParentTable for insert, update as
if (select count(*) from inserted) > 0
BEGIN
update mssql
END;

custom function sql for mssql 1;

custom function sql for mssql 2;

/* dbo.TestView1 */
if exists (select name from dbo.sysobjects where name = 'TestView1' and type = 'V')
   drop view dbo.TestView1;
create view dbo.TestView1 as
   select * from ParentTable;

/* dbo.TestView2 */
if exists (select name from dbo.sysobjects where name = 'TestView2' and type = 'V')
   drop view dbo.TestView2;
create view dbo.TestView2 as
   select * from mssql;

/* test.TestView1 */
if exists (select name from dbo.sysobjects where name = 'TestView1' and type = 'V')
   drop view test.TestView1;
create view test.TestView1 as
   select * from ParentTable;

custom procedure sql for mssql 1;

other bottom sql for mssql 1;

other bottom sql for mssql 2;

