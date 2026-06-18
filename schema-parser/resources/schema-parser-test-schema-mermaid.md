erDiagram
    PARENTTABLE {
        int ID PK
        varchar Name
        varchar Extra
        varchar Gender
    }
    CHILDTABLE {
        int ID PK
        int ParentID FK
        varchar Name
    }
    COLUMNTESTERTABLE {
        int sequence
        bigint longsequence
        tinyint byte
        smallint short
        int int
        bigint long
        float float
        double double
        decimal decimal
        boolean boolean
        date date
        datetime datetime
        time time
        timestamp timestamp
        char char
        varchar varchar
        varchar varcharWithCheck
        varchar enum
        text text
        binary binary
        uuid uuid
        json json
    }
    PROPERTY {
        int ID PK
        varchar Name
        varchar ShortName
        varchar Code
        varchar AltCode
        smallint NumberRooms
        int RegionID FK
    }
    REGION {
        int ID PK
        varchar Name
        varchar ShortName
        varchar Code
        boolean ExcludeFromCorpReports
    }
    KBI {
        int ID PK
        int PropertyID FK
        varchar Name
        varchar Code
        varchar ShowInModule
        int MasterKBICodeID FK
        int UnitID FK
    }
    MASTERKBICODE {
        int ID PK
        varchar Code
        varchar Description
        boolean ShowOnDashboard
        int SortOrder
        varchar GroupingFreeForm
    }
    UNIT {
        int ID PK
        int PropertyID FK
        varchar Name
        varchar SingularName
        varchar Symbol
        varchar Comment
    }
    CHILDTABLE }o--|| PARENTTABLE : "ParentID"
    PROPERTY }o--o| REGION : "RegionID"
    KBI }o--|| PROPERTY : "PropertyID"
    KBI }o--o| TEST.UNIT : "UnitID"
    KBI }o--o| MASTERKBICODE : "MasterKBICodeID"
    UNIT }o--|| PROPERTY : "PropertyID"
