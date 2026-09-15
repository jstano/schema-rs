if not exists (select 1 from sys.schemas where name = 'test')
   exec('create schema test')
GO

other top sql for mssql 1
GO

other top sql for mssql 2
GO

/* ChildTable */
if object_id('dbo.ChildTable', 'U') is not null
drop table dbo.ChildTable
GO

/* ColumnTesterTable */
if object_id('dbo.ColumnTesterTable', 'U') is not null
drop table dbo.ColumnTesterTable
GO

/* KBI */
if object_id('dbo.KBI', 'U') is not null
drop table dbo.KBI
GO

/* LongSequenceTesterTable */
if object_id('dbo.LongSequenceTesterTable', 'U') is not null
drop table dbo.LongSequenceTesterTable
GO

/* ParentTable */
if object_id('dbo.ParentTable', 'U') is not null
drop table dbo.ParentTable
GO

/* Unit */
if object_id('test.Unit', 'U') is not null
drop table test.Unit
GO

/* MasterKBICode */
if object_id('dbo.MasterKBICode', 'U') is not null
drop table dbo.MasterKBICode
GO

/* Property */
if object_id('dbo.Property', 'U') is not null
drop table dbo.Property
GO

/* Region */
if object_id('dbo.Region', 'U') is not null
drop table dbo.Region
GO

create table dbo.ChildTable
(
   ID integer identity(1,1) not null,
   ParentID integer not null,
   Name nvarchar(100) not null,
   constraint pk_childtable primary key (ID),
   constraint ak_childtable1 unique (ParentID,Name)
)
GO

alter table dbo.ChildTable set (lock_escalation = auto)
GO

create table dbo.ColumnTesterTable
(
   sequence integer identity(1,1) not null,
   byte tinyint,
   short smallint,
   int integer,
   long bigint,
   float real,
   double double precision,
   decimal decimal(19,4),
   boolean bit,
   date datetime,
   datetime datetime,
   time datetime,
   timestamp datetime,
   char nchar(1) constraint df_columntes_char_BF2D7A7C default default 'A',
   varchar nvarchar(10),
   varcharWithCheck nvarchar(6),
   enum nchar(1),
   text nvarchar(max),
   binary varbinary(max),
   uuid uniqueidentifier,
   json json,
   constraint ck_columntes_int_B3963409 check(int >= 1 and int <= 500),
   constraint ck_columntes_varcharwi_353F3BCB check(varcharWithCheck = 'ABC123'),
   constraint ck_columntes_enum_BF2E7C27 check(enum in ('1','2'))
)
GO

alter table dbo.ColumnTesterTable set (lock_escalation = disable)
GO

create table dbo.KBI
(
   ID integer identity(1,1) not null,
   PropertyID integer not null,
   Name nvarchar(50) not null,
   Code nvarchar(25) not null,
   ShowInModule nchar(1) not null,
   MasterKBICodeID integer,
   UnitID integer,
   constraint pk_kbi primary key (ID),
   constraint ak_kbi1 unique (PropertyID,Name),
   constraint ak_kbi2 unique (PropertyID,Code),
   constraint ck_kbi_showinmod_B47F96FB check(ShowInModule in ('A','B','L'))
)
GO

alter table dbo.KBI set (lock_escalation = auto)
GO

create index ix_kbi1 on dbo.KBI (MasterKBICodeID)
GO

create table dbo.LongSequenceTesterTable
(
   longsequence bigint identity(1,1) not null
)
GO

alter table dbo.LongSequenceTesterTable set (lock_escalation = auto)
GO

create table dbo.MasterKBICode
(
   ID integer identity(1,1) not null,
   Code nvarchar(25) not null,
   Description nvarchar(50) not null,
   ShowOnDashboard bit not null,
   SortOrder integer,
   GroupingFreeForm nvarchar(50),
   constraint pk_masterkbicode primary key (ID),
   constraint ak_masterkbicode1 unique (Code)
)
GO

alter table dbo.MasterKBICode set (lock_escalation = auto)
GO

create table dbo.ParentTable
(
   ID integer identity(1,1) not null,
   Name nvarchar(100) not null,
   Extra nvarchar(200),
   Gender nchar(1),
   constraint pk_parenttable primary key nonclustered (ID),
   constraint ak_parenttable1 unique clustered (Name,Extra),
   constraint ck_parenttab_gender_E250C9FC check(Gender in ('M','F'))
)
GO

alter table dbo.ParentTable set (lock_escalation = auto)
GO

create index ix_parenttable1 on dbo.ParentTable (Extra, Name) with (data_compression = page)
GO
create index ix_parenttable2 on dbo.ParentTable (ID, Name, Extra)
GO

insert into ParentTable (Name,Extra,Gender) values ('AAA','Extra AAA','M')
GO
insert into ParentTable (Name,Extra,Gender) values ('BBB','Extra BBB','F')
GO
insert into ParentTable (Name,Extra,Gender) values ('MSSQL','Extra MSSQL','F')
GO

create table dbo.Property
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
   constraint ck_property_numberroo_90DF89E5 check(NumberRooms >= 0 and NumberRooms <= 20000)
)
GO

alter table dbo.Property set (lock_escalation = auto)
GO

create table dbo.Region
(
   ID integer identity(1,1) not null,
   Name nvarchar(50) not null,
   ShortName nvarchar(25) not null,
   Code nvarchar(25) not null,
   ExcludeFromCorpReports bit not null,
   constraint pk_region primary key (ID),
   constraint ak_region1 unique (Name),
   constraint ak_region2 unique (Code)
)
GO

alter table dbo.Region set (lock_escalation = auto)
GO

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
)
GO

alter table test.Unit set (lock_escalation = auto)
GO

/* relations */
alter table dbo.ChildTable add constraint fk_childtable1 foreign key (ParentID) references dbo.ParentTable(ID) on delete cascade
GO
alter table dbo.KBI add constraint fk_kbi1 foreign key (PropertyID) references dbo.Property(ID) on delete cascade
GO
alter table dbo.KBI add constraint fk_kbi2 foreign key (UnitID) references test.Unit(ID) on delete set null
GO
alter table dbo.KBI add constraint fk_kbi3 foreign key (MasterKBICodeID) references dbo.MasterKBICode(ID) on delete set null
GO
alter table dbo.Property add constraint fk_property1 foreign key (RegionID) references dbo.Region(ID) on delete set null
GO
alter table test.Unit add constraint fk_unit1 foreign key (PropertyID) references dbo.Property(ID) on delete cascade
GO

/* parenttable_delete */
if object_id('dbo.parenttable_delete', 'TR') is not null
   drop trigger dbo.parenttable_delete
GO

create trigger parenttable_delete on dbo.ParentTable for delete as
if (select count(*) from deleted) > 0
BEGIN
delete from mssql
END
GO

/* parenttable_update */
if object_id('dbo.parenttable_update', 'TR') is not null
   drop trigger dbo.parenttable_update
GO

create trigger parenttable_update on dbo.ParentTable for insert, update as
if (select count(*) from inserted) > 0
BEGIN
update mssql
END
GO

if exists (select * from dbo.sysobjects where id = object_id(N'[dbo].[customFunction1]') and objectproperty(id, N'IsScalarFunction') = 1)
drop function dbo.customFunction1
GO
custom function sql for mssql 1
GO

if exists (select * from dbo.sysobjects where id = object_id(N'[dbo].[customFunction2]') and objectproperty(id, N'IsScalarFunction') = 1)
drop function dbo.customFunction2
GO
custom function sql for mssql 2
GO

/* dbo.TestView1 */
if object_id('dbo.TestView1', 'V') is not null
   drop view dbo.TestView1
GO
create view dbo.TestView1 as
   select * from ParentTable
GO

/* dbo.TestView2 */
if object_id('dbo.TestView2', 'V') is not null
   drop view dbo.TestView2
GO
create view dbo.TestView2 as
   select * from mssql
GO

/* test.TestView1 */
if object_id('test.TestView1', 'V') is not null
   drop view test.TestView1
GO
create view test.TestView1 as
   select * from ParentTable
GO

if exists (select * from dbo.sysobjects where id = object_id(N'[dbo].[customProcedure1]') and objectproperty(id, N'IsProcedure') = 1)
drop procedure dbo.customProcedure1
GO
custom procedure sql for mssql 1
GO

other bottom sql for mssql 1
GO

other bottom sql for mssql 2
GO

