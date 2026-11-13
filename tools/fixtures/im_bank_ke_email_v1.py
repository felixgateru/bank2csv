#!/usr/bin/env python3
"""Generates a synthetic I&M Bank (Kenya) `email_v1` RawDocument fixture.

Coordinates, offsets and quirks copy a real statement; every name, number and
amount is made up. Run from the repo root:

    python3 tools/fixtures/im_bank_ke_email_v1.py

Writes fixtures/im_bank_ke/email_v1/input.rawdoc.json and expected.csv.
"""
import json
import pathlib

CHAR_W = 4.6  # approximate advance of a size-8 Lato digit, in points
OUT = pathlib.Path("fixtures/im_bank_ke/email_v1")

# Right edges of right-aligned columns, from the real layout.
WITHDRAWAL_X1, DEPOSIT_X1, BALANCE_X1 = 280.2, 359.1, 448.8
NARRATIVE_X0 = 456.8


def span(text, x0, y, size=8.0, x1=None):
    return {"text": text, "x0": x0, "x1": x1 if x1 is not None else x0 + len(text) * CHAR_W, "y": y, "font_size": size}


def right(text, x1, y):
    return span(text, x1 - len(text) * CHAR_W, y, x1=x1)


def fmt(minor):
    sign = "-" if minor < 0 else ""
    minor = abs(minor)
    return f"{sign}{minor // 100:,}.{minor % 100:02d}"


def plain(minor):
    sign = "-" if minor < 0 else ""
    minor = abs(minor)
    return f"{sign}{minor // 100}.{minor % 100:02d}"


def account_block(y0, page_no, pages):
    return [
        span("Account Name", 16.5, y0, 9.0),
        span("JANE DOE - KES ACC", 110.2, y0, 9.0),
        span(f"Page {page_no} Of {pages}", 499.5, y0, 9.0),
        span("Account Number", 16.5, y0 + 15, 9.0),
        span("001XXXXXXX0001", 110.2, y0 + 15, 9.0),
        span("Account Type", 16.5, y0 + 30, 9.0),
        span("PAYGO-LCY", 110.2, y0 + 30, 9.0),
        span("Statement Period", 357.0, y0 + 40.5, 9.0),
        span("01-10-2026", 441.8, y0 + 40.5, 9.0),
        span("To", 501.8, y0 + 40.5, 9.0),
        span("31-10-2026", 524.2, y0 + 40.5, 9.0),
        span("Account Currency", 16.5, y0 + 45, 9.0),
        span("KENYAN SHILLINGS", 110.2, y0 + 45, 9.0),
    ]


def header(y):
    return [
        span("Tran Date", 17.5, y, 9.0, 62.3),
        span("Value Date", 76.0, y, 9.0, 125.9),
        span("Ref No", 153.3, y, 9.0, 183.9),
        span("Withdrawals", 222.3, y, 9.0, 280.2),
        span("Deposits", 319.6, y, 9.0, 359.1),
        span("Balance", 402.5, y, 9.0, 437.9),
        span("Transaction Narrative", 456.8, y, 9.0, 555.5),
    ]


def footer():
    return [
        span("*********** This is a System Generated Email Attachment ***********", 209.3, 821.9, 6.0),
        span("© 2026 I&M Bank Electronic Messaging System. All rights reserved.", 207.1, 829.0, 6.0),
    ]


# (date, value_date, amount_minor, narrative_lines). Narrative is placed like the
# real layout: one line → same baseline; two → above/below; three → above/same/below.
OPENING = 5_000_000
TXNS_PAGE1 = [
    ("01-10-26", "01-10-26", -1_000_000, ["0700000001/MPESA Payment", "to 0700000001"]),
    ("03-10-26", "02-10-26", -250_050, ["SUPERMARKET ONE", "NAIROB1003 000001PRCR0001"]),
    ("05-10-26", "05-10-26", -10_000, ["EXCISE DUTY"]),
    ("09-10-26", "09-10-26", 2_500_000, ["JOHN SMITH", "DOE/EXAMPLE PERSON", "SALARY/Oct"]),
    ("12-10-26", "12-10-26", -795, ["Payment to 000000/Excise D"]),
]
TXNS_PAGE2 = [
    ("20-10-26", "20-10-26", 1_234_567, ["CTS/EFT CR EXAMPLE CORP LTD"]),
    ("28-10-26", "28-10-26", -3_000_000, ["CTS/EFT DR EXAMPLE", "SACCO"]),
]


def rows(txns, y, balance):
    spans, csv = [], []
    for date, vdate, amount, narrative in txns:
        balance += amount
        spans += [span(date, 17.5, y), span(vdate, 78.8, y)]
        if amount < 0:
            spans.append(right(fmt(-amount), WITHDRAWAL_X1, y))
        else:
            spans.append(right(fmt(amount), DEPOSIT_X1, y))
        spans.append(right(fmt(balance) + "Cr", BALANCE_X1, y))
        offsets = {1: [0.0], 2: [-4.7, 4.8], 3: [-9.5, 0.0, 9.5]}[len(narrative)]
        for off, text in zip(offsets, narrative):
            spans.append(span(text, NARRATIVE_X0, round(y + off, 1)))
        debit = plain(-amount) if amount < 0 else ""
        credit = plain(amount) if amount >= 0 else ""
        iso = lambda d: f"20{d[6:8]}-{d[3:5]}-{d[0:2]}"
        csv.append(",".join([iso(date), iso(vdate), " ".join(narrative), "", debit, credit, plain(amount), plain(balance)]))
        y += 25.0
    return spans, csv, y, balance


def main():
    pages, csv = [], []

    # Page 1: invisible text above the letterhead, address block, table.
    p1 = [
        span(f"{OPENING / 100:.2f}", 148.5, 18.4, 1.0),
        span("12345678", 50.2, 25.9, 1.0),
        span("31-10-2026", 202.5, 25.9, 1.0),
        span("Account Statement", 17.5, 56.5, 16.0),
        span("JANE DOE", 17.5, 91.8, 10.0),
        span("EXAMPLE STREET", 498.4, 91.8, 10.0),
    ]
    p1 += account_block(185.1, 1, 3)
    p1 += header(250.1)
    p1 += [span("01-10-26", 17.5, 267.2), right(fmt(OPENING) + "Cr", BALANCE_X1, 267.2), span("B/F", NARRATIVE_X0, 267.2)]
    s, c, _, balance = rows(TXNS_PAGE1, 292.0, OPENING)
    p1 += s + footer()
    csv += c
    pages.append({"number": 1, "width": 595.0, "height": 842.0, "spans": p1})

    # Page 2: repeated account block and header, rows, totals, closing balance.
    p2 = account_block(35.8, 2, 3) + header(101.6)
    s, c, y, balance = rows(TXNS_PAGE2, 123.2, balance)
    p2 += s
    csv += c
    all_txns = TXNS_PAGE1 + TXNS_PAGE2
    total_w = sum(-a for _, _, a, _ in all_txns if a < 0)
    total_d = sum(a for _, _, a, _ in all_txns if a >= 0)
    p2 += [span("Total", 78.8, y), right(fmt(total_w), WITHDRAWAL_X1, y), right(fmt(total_d), DEPOSIT_X1, y)]
    p2 += [span("Balance as of", 144.7, y + 22.7, 10.0), span("31-10-2026", 272.5, y + 22.7, 10.0),
           span(fmt(balance) + "Cr", 370.9, y + 22.7, 10.0)]
    p2 += [span("Cleared Balance as on", 109.4, y + 45.4, 10.0), span("01-11-2026", 272.5, y + 45.4, 10.0),
           span(fmt(balance) + "Cr", 370.9, y + 45.4, 10.0)]
    p2 += footer()
    pages.append({"number": 2, "width": 595.0, "height": 842.0, "spans": p2})

    # Page 3: promotions only, no table.
    p3 = account_block(35.8, 3, 3) + [span("1.Free transfers! Download the App today. #OnYourSide", 17.5, 99.6, 9.0)] + footer()
    pages.append({"number": 3, "width": 595.0, "height": 842.0, "spans": p3})

    for page in pages:
        page["spans"].sort(key=lambda s: (s["y"], s["x0"]))

    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "input.rawdoc.json").write_text(json.dumps({"kind": "spans", "pages": pages}, indent=1, ensure_ascii=False) + "\n")
    header_row = "date,value_date,description,reference,debit,credit,amount,balance"
    (OUT / "expected.csv").write_text("\r\n".join([header_row] + csv) + "\r\n")
    print(f"wrote {OUT}/ ({len(csv)} transactions, closing {plain(balance)})")


if __name__ == "__main__":
    main()
