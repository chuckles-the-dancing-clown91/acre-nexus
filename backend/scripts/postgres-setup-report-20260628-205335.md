# PostgreSQL setup report — 20260628-205335

- Host: `customer.ashnvax2.isp.starlink.com`  ·  OS: `Linux`  ·  Package manager: `dnf`
- PostgreSQL server version: `18.3`
- Listen: `localhost:5432`  ·  Auth: `scram-sha-256`

## Packages installed
_(none — already installed)_

## Services enabled
_(none)_

## Config files changed
- /var/lib/pgsql/data/pg_hba.conf (backup: /var/lib/pgsql/data/pg_hba.conf.acre-bak-20260628-205335)

## Databases created
- acre_user (owner=acre_user_owner, app=acre_user_app)
- acre_property (owner=acre_property_owner, app=acre_property_app)
- acre_client (owner=acre_client_owner, app=acre_client_app)

## Roles created/updated
- acre_user_owner
- acre_user_app
- acre_property_owner
- acre_property_app
- acre_client_owner
- acre_client_app

## Secret / env files (mode 0600 — NOT committed)
- /home/cwilson/Documents/Dev/acre-nexus/backend/scripts/secrets/user.env
- /home/cwilson/Documents/Dev/acre-nexus/backend/scripts/secrets/property.env
- /home/cwilson/Documents/Dev/acre-nexus/backend/scripts/secrets/client.env
- /home/cwilson/Documents/Dev/acre-nexus/backend/scripts/secrets/acre.env (combined — the file the app needs; copy to backend/.env)
- /home/cwilson/Documents/Dev/acre-nexus/backend/scripts/../.env (DB connection strings installed)

## Notes
- Wrote DB connection strings to backend/.env (existing file backed up; non-DB settings preserved).
- Ensured pg_hba scram rules on loopback + password_encryption=scram-sha-256 (existing listen_addresses/port left unchanged).

## Live cluster inventory
```
                                                                        List of databases
     Name      |        Owner        | Encoding | Locale Provider |   Collate   |    Ctype    | Locale | ICU Rules |              Access privileges              
---------------+---------------------+----------+-----------------+-------------+-------------+--------+-----------+---------------------------------------------
 acre_client   | acre_client_owner   | UTF8     | libc            | en_US.UTF-8 | en_US.UTF-8 |        |           | acre_client_owner=CTc/acre_client_owner    +
               |                     |          |                 |             |             |        |           | acre_client_app=c/acre_client_owner
 acre_property | acre_property_owner | UTF8     | libc            | en_US.UTF-8 | en_US.UTF-8 |        |           | acre_property_owner=CTc/acre_property_owner+
               |                     |          |                 |             |             |        |           | acre_property_app=c/acre_property_owner
 acre_user     | acre_user_owner     | UTF8     | libc            | en_US.UTF-8 | en_US.UTF-8 |        |           | acre_user_owner=CTc/acre_user_owner        +
               |                     |          |                 |             |             |        |           | acre_user_app=c/acre_user_owner
 postgres      | postgres            | UTF8     | libc            | en_US.UTF-8 | en_US.UTF-8 |        |           | 
 template0     | postgres            | UTF8     | libc            | en_US.UTF-8 | en_US.UTF-8 |        |           | =c/postgres                                +
               |                     |          |                 |             |             |        |           | postgres=CTc/postgres
 template1     | postgres            | UTF8     | libc            | en_US.UTF-8 | en_US.UTF-8 |        |           | =c/postgres                                +
               |                     |          |                 |             |             |        |           | postgres=CTc/postgres
(6 rows)


                                  List of roles
      Role name      |                         Attributes                         
---------------------+------------------------------------------------------------
 acre_client_app     | 
 acre_client_owner   | 
 acre_property_app   | 
 acre_property_owner | 
 acre_user_app       | 
 acre_user_owner     | 
 postgres            | Superuser, Create role, Create DB, Replication, Bypass RLS
```

## How to undo
- Restore config: copy each `*.acre-bak-20260628-205335` back over its original and restart the service.
- Drop databases/roles:
```sql
DROP DATABASE IF EXISTS "acre_user";
DROP DATABASE IF EXISTS "acre_property";
DROP DATABASE IF EXISTS "acre_client";
DROP ROLE IF EXISTS "acre_user_owner";
DROP ROLE IF EXISTS "acre_user_app";
DROP ROLE IF EXISTS "acre_property_owner";
DROP ROLE IF EXISTS "acre_property_app";
DROP ROLE IF EXISTS "acre_client_owner";
DROP ROLE IF EXISTS "acre_client_app";
```
- Remove packages: `sudo dnf remove postgresql*` (purges nothing under the data dir).
