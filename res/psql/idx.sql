
create index on obj.doc (fid);
create index on obj.doc (id);

create index on obj.fts (fid);
create index on obj.fts (id);

create index on obj.fts using gin(doc);
create index on obj.doc using gin(attr);

-- TODO
--create index on obj.doc using gin(attr -> 'tag');

create index on obj.doc ((attr ->> 'name'));

create index on obj.file (id, obsolete);

create index on obj.anno (id, obsolete);
create index on obj.anno using gin(pid);
create index on obj.anno (fid);

create index on obj.anno (id, modified);

create index on sel.tmp(id, sel);
create index on sel.pers(id, sel);
