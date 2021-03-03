-- trigger
-- 1. anno is immutable(except obsolete filed)
-- 2. aid attr & doc is updatable for file
-- 3. func is_tip -> is anno and is not the parent of any other anno      
-- 4. when anno insert(anno is inmmuable)
--   + if is tip, then update obsolete field, and parent obsolete field
---  + then for this and parent anno ref-ed file:
--     * update ref-ed file, set aid to all anno with fid is oid and not obsolete
--     * update ref-ed file, when has any aid, mark obsolete false, else true

drop table if exists log.dat_op;

create table log.dat_op (op text, id bytea);

CREATE OR REPLACE FUNCTION obj.anno_trig_func() RETURNS TRIGGER AS $body$
BEGIN
    IF (TG_OP = 'UPDATE') THEN
         IF (OLD.id != NEW.id) THEN
             RAISE EXCEPTION 'id should not change';
         END IF;

       -- for ANNO
        IF (OLD.fid != NEW.fid) OR
           (OLD.pid != NEW.pid) THEN
             raise exception 'only obsolete field for anno is mutable';
        END IF;

        if (OLD.obsolete != NEW.obsolete) then
           update obj.file as d1
             set aid = array(select id from obj.anno
                                 where fid = d1.id and not obsolete)
             where id = NEW.fid;
        end if;

        return NEW;
    end if;


    if (TG_OP = 'DELETE') and (OLD.obsolete = False) THEN
        raise exception 'can not delete non obsolete field';
    end if;


    if (TG_OP = 'INSERT') THEN
        update obj.anno as d1
            set obsolete = exists (select id from obj.anno
                                             where NEW.id = any(pid))
            where id = NEW.id;
        update obj.anno set obsolete = True where id = any(NEW.pid);

        --insert into log.dat_op values('ins', NEW.id);

        return NEW;
    end if;

    return NULL;
end;
$body$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION obj.anno_trig_func1() RETURNS TRIGGER AS $body$
BEGIN
    NEW.modified := now();

    return NEW;
end;
$body$ LANGUAGE plpgsql;


CREATE OR REPLACE FUNCTION obj.file_trig_func() RETURNS TRIGGER AS $body$
BEGIN
    IF (TG_OP = 'UPDATE') THEN
        IF (OLD.id != NEW.id) THEN
            RAISE EXCEPTION 'id should not change';
        END IF;

        if (OLD.aid != NEW.aid) then
           update obj.file
             set obsolete = (aid = '{}')
             where id = NEW.id;
        end if;
        
        --insert into log.dat_op values('up', NEW.id);

        return NEW;
    end if;


    if (TG_OP = 'DELETE') and (OLD.obsolete = False) THEN
        raise exception 'can not delete non obsolete field';
    end if;


    if (TG_OP = 'INSERT') THEN
        update obj.file as d1
            set aid = array(select id from obj.anno
                                 where fid = d1.id and not obsolete)
            where id = NEW.id;
            
        --insert into log.dat_op values('ins', NEW.id);

        return NEW;
    end if;

    return NULL;
end;
$body$ LANGUAGE plpgsql;



drop trigger if exists obj_anno_trig on obj.anno;
create trigger obj_anno_trig
    after insert or update or delete ON obj.anno
    for each row execute procedure obj.anno_trig_func();

drop trigger if exists obj_anno_trig1 on obj.anno;
create trigger obj_anno_trig1
    before insert or update ON obj.anno
    for each row execute procedure obj.anno_trig_func1();


drop trigger if exists obj_file_trig on obj.file;
create trigger obj_file_trig
    after insert or update or delete ON obj.file
    for each row execute procedure obj.file_trig_func();

-- NOTE, always insert anno before insert doc
CREATE OR REPLACE FUNCTION obj.up_fid_func() RETURNS TRIGGER AS $body$
DECLARE
  v_fid bytea := null;
BEGIN
    IF (TG_OP = 'UPDATE') THEN
         IF (OLD.id != NEW.id) THEN
             RAISE EXCEPTION 'id should not change';
         END IF;

         return NEW;
    end if;
    
    if (TG_OP = 'INSERT') THEN
        select fid into v_fid from obj.anno where id = NEW.id limit 1;

        if v_fid is null then
           v_fid = NEW.id;
        end if;

        update obj.doc set fid = v_fid where id = NEW.id;
        update obj.fts set fid = v_fid where id = NEW.id;

        return new;
    end if;

    return null;
end;
$body$ LANGUAGE plpgsql;


drop trigger if exists obj_doc_trig on obj.doc;
create trigger obj_doc_trig
    after insert or update or delete ON obj.doc
    for each row execute procedure obj.up_fid_func();

drop trigger if exists obj_fts_trig on obj.fts;
create trigger obj_fts_trig
    after insert or update or delete ON obj.fts
    for each row execute procedure obj.up_fid_func();
