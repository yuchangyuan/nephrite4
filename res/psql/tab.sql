create schema if not exists  obj;

drop table if exists obj.anno;
create table obj.anno (
       id bytea primary key,

       -- parent id, is history parenet of the anno
       pid bytea[] not null default '{}',

       -- file id for anno, link to obj.file.id
       fid bytea not null,

       -- generated
       obsolete bool not null default true,
       modified timestamptz not null default now()
);

drop table if exists obj.file;
create table obj.file (
       id bytea primary key,

       -- generated, default value should match aid
       obsolete bool not null default true,

       -- inverse of 'fid', ref to anno, auto update by trigger, empty for anno
       aid bytea[] not null default '{}'
);

-- multiple meta for 
drop table if exists obj.doc;
create table obj.doc (
       id bytea not null, -- id of anno or file
       fid bytea, -- file id, when id point to anno
       attr jsonb not null default '{}' -- NOTE: full content discarded
);

drop table if exists obj.fts;
create table obj.fts (
       id bytea not null,
       fid bytea, -- file id
       rel int8 not null default 0, -- offset
       doc tsvector not null default ''
);


create schema if not exists sel;


drop table if exists sel.tmp;
create table sel.tmp (
  sel text not null,
  id bytea not null,
  primary key(sel, id)
);

drop table if exists sel.pers;
create table sel.pers (
  sel text not null,
  id bytea not null,
  primary key (sel, id)
);

create schema if not exists log;

--create extension pg_trgm;
--create extension intarray;

--create index on m2 (oid);
--create index ON m2 using gin (kv gin_trgm_ops, kn gin_trgm_ops);

set search_path to public, log, sel, obj;
