import os
import mysql.connector
import xml.etree.ElementTree as ET
from dotenv import load_dotenv

load_dotenv()

# Database connection setup using environment variables
db_config = {
    "host": os.getenv("DB_HOST"),
    "user": os.getenv("DB_USER"),
    "password": os.getenv("DB_PASSWORD"),
    "database": os.getenv("DB_NAME"),
}

# Path to save the sitemap file
sitemap_path = "../forge/blog/sitemap.xml"

try:
    # Establish a connection to the MySQL database
    conn = mysql.connector.connect(**db_config)
    cursor = conn.cursor(dictionary=True)

    # Query to select published posts for the sitemap
    query = """
    SELECT slug, date_published, date_updated
    FROM posts
    WHERE published = 1
    ORDER BY date_published DESC;
    """
    cursor.execute(query)
    posts = cursor.fetchall()

    # Start building the XML structure
    urlset = ET.Element("urlset", xmlns="http://www.sitemaps.org/schemas/sitemap/0.9")

    for post in posts:
        url = ET.SubElement(urlset, "url")
        loc = ET.SubElement(url, "loc")
        lastmod = ET.SubElement(url, "lastmod")

        # Construct the URL from the slug
        loc.text = f"https://jkcoxson.com/blog/{post['slug']}"

        # Use date_updated if available, otherwise fall back to date_published
        date = post["date_updated"] or post["date_published"]
        lastmod.text = date.strftime("%Y-%m-%d")

    # Convert XML structure to a string
    sitemap_data = ET.tostring(urlset, encoding="unicode", method="xml")

    # Write XML data to the sitemap file
    with open(sitemap_path, "w+") as f:
        f.write(sitemap_data)

    print(f"Sitemap successfully created at {sitemap_path}")

    # Close database connection
    if conn.is_connected():
        cursor.close()
        conn.close()

except mysql.connector.Error as err:
    print(f"Error: {err}")
