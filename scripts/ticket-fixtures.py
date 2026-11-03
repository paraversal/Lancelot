#!/usr/bin/env python3
"""Writes the ticket sheet fixtures in fixtures/tickets (roadmap M7).

A series of imports of the same made-up event, in the association's layout: one row per ticket with
the buyer's name and e-mail, which Lancelot must never read (R-TKT-17).

  import-1.ods          first import
  import-2.ods          later: new orders, order 4711 cancelled, 4702 upgraded to Supporter,
                        a row without an order ID and a blank row
  import-3.ods          import 2 plus two tickets of an unknown tier, "Soli-Ticket"
  import-1.xlsx/.csv    import 1 in the other formats; order IDs as numbers in .xlsx

Usage: scripts/ticket-fixtures.py (no dependencies; the zip files are byte-for-byte stable).
"""

import csv
import io
import zipfile
from pathlib import Path
from xml.sax.saxutils import escape

OUT = Path(__file__).resolve().parent.parent / "fixtures" / "tickets"
HEADER = ["Order ID", "Name", "E-Mail", "Ticket"]


def person(order, n):
    return [f"Gast {order}-{n}", f"gast{order}.{n}@example.invalid"]


def rows(orders):
    out = []
    for order, tickets in orders:
        for n, tier in enumerate(tickets, 1):
            out.append([order, *person(order, n), tier])
    return out


IMPORT_1 = rows(
    [
        (4701, ["Early Bird", "Early Bird"]),
        (4702, ["Regular"]),
        (4711, ["Regular", "Regular"]),
        (4802, ["Supporter"]),
    ]
)
IMPORT_2 = rows(
    [
        (4701, ["Early Bird", "Early Bird"]),
        (4702, ["Supporter"]),
        (4802, ["Supporter"]),
        (4803, ["Regular", "Regular", "Regular"]),
        (4810, ["regular"]),
    ]
) + [["", "Gast ohne Bestellung", "x@example.invalid", "Regular"], ["", "", "", ""]]
IMPORT_3 = IMPORT_2[:-2] + rows([(4820, ["Soli-Ticket", "Soli-Ticket"])])

STAMP = (2026, 6, 1, 0, 0, 0)


def write_zip(path, files):
    with zipfile.ZipFile(path, "w") as z:
        for name, data in files:
            info = zipfile.ZipInfo(name, STAMP)
            info.compress_type = zipfile.ZIP_STORED if name == "mimetype" else zipfile.ZIP_DEFLATED
            z.writestr(info, data)


def ods(path, table):
    def cell(v):
        if isinstance(v, int):
            return f'<table:table-cell office:value-type="float" office:value="{v}"><text:p>{v}</text:p></table:table-cell>'
        if v == "":
            return "<table:table-cell/>"
        return f'<table:table-cell office:value-type="string"><text:p>{escape(v)}</text:p></table:table-cell>'

    body = "".join(
        "<table:table-row>" + "".join(cell(v) for v in row) + "</table:table-row>"
        for row in [HEADER, *table]
    )
    content = (
        '<?xml version="1.0" encoding="UTF-8"?>'
        '<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"'
        ' xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0"'
        ' xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" office:version="1.3">'
        '<office:body><office:spreadsheet><table:table table:name="Tickets">'
        f"{body}</table:table></office:spreadsheet></office:body></office:document-content>"
    )
    manifest = (
        '<?xml version="1.0" encoding="UTF-8"?>'
        '<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.3">'
        '<manifest:file-entry manifest:full-path="/" manifest:media-type="application/vnd.oasis.opendocument.spreadsheet"/>'
        '<manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/>'
        "</manifest:manifest>"
    )
    write_zip(
        path,
        [
            ("mimetype", "application/vnd.oasis.opendocument.spreadsheet"),
            ("META-INF/manifest.xml", manifest),
            ("content.xml", content),
        ],
    )


def xlsx(path, table):
    def col(i):
        return chr(ord("A") + i)

    def cell(v, ref):
        if isinstance(v, int):
            return f'<c r="{ref}"><v>{v}</v></c>'
        return f'<c r="{ref}" t="inlineStr"><is><t>{escape(v)}</t></is></c>'

    sheet_rows = "".join(
        f'<row r="{r}">' + "".join(cell(v, f"{col(i)}{r}") for i, v in enumerate(row)) + "</row>"
        for r, row in enumerate([HEADER, *table], 1)
    )
    ns = 'xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"'
    rel = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
    write_zip(
        path,
        [
            (
                "[Content_Types].xml",
                '<?xml version="1.0" encoding="UTF-8"?>'
                '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
                '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
                '<Default Extension="xml" ContentType="application/xml"/>'
                '<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>'
                '<Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>'
                "</Types>",
            ),
            (
                "_rels/.rels",
                '<?xml version="1.0" encoding="UTF-8"?>'
                '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
                f'<Relationship Id="rId1" Type="{rel}/officeDocument" Target="xl/workbook.xml"/>'
                "</Relationships>",
            ),
            (
                "xl/workbook.xml",
                f'<?xml version="1.0" encoding="UTF-8"?><workbook {ns} xmlns:r="{rel}">'
                '<sheets><sheet name="Tickets" sheetId="1" r:id="rId1"/></sheets></workbook>',
            ),
            (
                "xl/_rels/workbook.xml.rels",
                '<?xml version="1.0" encoding="UTF-8"?>'
                '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
                f'<Relationship Id="rId1" Type="{rel}/worksheet" Target="worksheets/sheet1.xml"/>'
                "</Relationships>",
            ),
            (
                "xl/worksheets/sheet1.xml",
                f'<?xml version="1.0" encoding="UTF-8"?><worksheet {ns}><sheetData>{sheet_rows}</sheetData></worksheet>',
            ),
        ],
    )


def write_csv(path, table):
    out = io.StringIO()
    csv.writer(out, delimiter=";", lineterminator="\r\n").writerows([HEADER, *table])
    path.write_text(out.getvalue(), encoding="utf-8")


if __name__ == "__main__":
    OUT.mkdir(parents=True, exist_ok=True)
    ods(OUT / "import-1.ods", IMPORT_1)
    ods(OUT / "import-2.ods", IMPORT_2)
    ods(OUT / "import-3.ods", IMPORT_3)
    xlsx(OUT / "import-1.xlsx", IMPORT_1)
    write_csv(OUT / "import-1.csv", IMPORT_1)
