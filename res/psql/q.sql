
select distinct x.id, x.fid from
  (select distinct id, rid from obj.anno where obsolete = false) as x join
  lateral (select distinct fid from obj.doc where attr ->> 'type' like 'image%') as y0 on y0.fid = x.fid join
  lateral (select distinct fid from obj.fts where to_tsquery('h:*') @@ doc) as y1 on y1.fid = x.fid join
  lateral (select distinct fid from obj.fts where to_tsquery('jpg') @@ doc) as y2 on y2.fid = x.fid join
  lateral (select distinct fid from obj.fts where to_tsquery('a:*|b:*|e:*|g:*|h:*') @@ doc) as y3 on y3.fid = x.fid

select distinct x0.fid from obj.fts as x0 where Q0

select distinct x1.fid from obj.fts as x1,
(select distinct x0.fid from obj.fts as x0 where Q0) as y0 where x1.fid = y0.fid and Q1


select distinct x2.fid from obj.fts as x2,
(select distinct x1.fid from obj.fts as x1,
(select distinct x0.fid from obj.fts as x0 where Q0) as y0 where x1.fid = y0.fid and Q1) as y1 where x2.fid = y1.fid and Q2

select distinct x2.fid from obj.fts as x2,
(select distinct x1.fid from obj.fts as x1,
(select distinct x0.fid from obj.fts as x0 where to_tsquery('a:*|b:*|e:*|h:*|g:*') @@ x0.doc) as y0 where x1.fid = y0.fid and to_tsquery('h:*') @@ x1.doc) as y1 where x2.fid = y1.fid and to_tsquery('h:*') @@ x2.doc;

select z1.* from obj.doc as z0, lateral (select * from jsonb_each_text(z0.attr) where "key" ilike 'X%p%' and ("value" ilike '%"%par%"%' or "value" ilike '%par%')) as z1;

-- 1. '+.*', '-.*" -> tag search
-- 2. '.*:.*' -> attr search
--   a) no patt match on key, patt/no patt match on value
--   b) patt match on key, patt/no patt match on value
--   c) all patt match us 'ilike', accept '%' & '_'
-- 3. fts search, accept prefix match, use '%' as unknown suffix

-- 'ab & cd' -> to_tsquery('ab & cd') @@ doc
-- 'name:*jpg' -> attr ->> 'name' ilike '%jpg'
-- '*am*:jpg' -> "key" ilike '*am*' and "value" = 'jpg'
-- '+abc' -> attr ->> 'tag' = 'abc' or attr ->> 'tag' like '%"abc"%'
-- '-abc' -> group by fid having not bool_or(attr ->> 'tag' = 'abc' or attr ->> 'tag' like '%"abc"%')

-- get dir
select distinct (get_byte(t.id, 0) * 256 + get_byte(t.id, 1)) / 64 from (select id from obj.anno union select id from obj.file) as t;
