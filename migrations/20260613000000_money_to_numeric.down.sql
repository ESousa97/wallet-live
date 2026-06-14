ALTER TABLE assets
    ALTER COLUMN unit_value TYPE DOUBLE PRECISION USING unit_value::double precision;
