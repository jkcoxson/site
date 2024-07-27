-- Jackson Coxson
-- List of tables used for jkcoxson.com

CREATE TABLE posts (
    slug VARCHAR(255) NOT NULL,
    post_name VARCHAR(255) NOT NULL,
    file_path VARCHAR(255) NOT NULL,
    sneak_peak VARCHAR(255),
    image_path VARCHAR(255),
    published TINYINT,
    date_published DATETIME NOT NULL,
    date_updated DATETIME,
    category INT,
    PRIMARY KEY (slug)
);

CREATE TABLE categories (
    id INT NOT NULL AUTO_INCREMENT,
    category_name VARCHAR(255) NOT NULL,
    PRIMARY KEY (id)
);

CREATE TABLE tags (
    id INT NOT NULL AUTO_INCREMENT,
    tag_name VARCHAR(255) NOT NULL,
    PRIMARY KEY (id)
);

CREATE TABLE post_tags (
    id INT NOT NULL AUTO_INCREMENT,
    slug VARCHAR(255) NOT NULL,
    tag_id INT NOT NULL,
    PRIMARY KEY (id)
);
