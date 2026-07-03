@startuml
entity PARENTTABLE {
  * ID : int <<PK>>
  --
  Name : varchar
  Extra : varchar
  Gender : varchar
}

entity CHILDTABLE {
  * ID : int <<PK>>
  --
  ParentID : int <<FK>>
  Name : varchar
}

entity COLUMNTESTERTABLE {
  sequence : int
  longsequence : bigint
  byte : tinyint
  short : smallint
  int : int
  long : bigint
  float : float
  double : double
  decimal : decimal
  boolean : boolean
  date : date
  datetime : datetime
  time : time
  timestamp : timestamp
  char : char
  varchar : varchar
  varcharWithCheck : varchar
  enum : varchar
  text : text
  binary : binary
  uuid : uuid
  json : json
}

entity PROPERTY {
  * ID : int <<PK>>
  --
  Name : varchar
  ShortName : varchar
  Code : varchar
  AltCode : varchar
  NumberRooms : smallint
  RegionID : int <<FK>>
}

entity REGION {
  * ID : int <<PK>>
  --
  Name : varchar
  ShortName : varchar
  Code : varchar
  ExcludeFromCorpReports : boolean
}

entity KBI {
  * ID : int <<PK>>
  --
  PropertyID : int <<FK>>
  Name : varchar
  Code : varchar
  ShowInModule : varchar
  MasterKBICodeID : int <<FK>>
  UnitID : int <<FK>>
}

entity MASTERKBICODE {
  * ID : int <<PK>>
  --
  Code : varchar
  Description : varchar
  ShowOnDashboard : boolean
  SortOrder : int
  GroupingFreeForm : varchar
}

entity UNIT {
  * ID : int <<PK>>
  --
  PropertyID : int <<FK>>
  Name : varchar
  SingularName : varchar
  Symbol : varchar
  Comment : varchar
}

CHILDTABLE }o--|| PARENTTABLE : ParentID
PROPERTY }o--o| REGION : RegionID
KBI }o--|| PROPERTY : PropertyID
KBI }o--o| TEST.UNIT : UnitID
KBI }o--o| MASTERKBICODE : MasterKBICodeID
UNIT }o--|| PROPERTY : PropertyID
@enduml
