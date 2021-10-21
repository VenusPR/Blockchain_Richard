import binascii

with open('trove_account.key', 'wb') as fout:
    fout.write(
        binascii.unhexlify('2049aea4dfc9e574112aa64110ad052a7693dbf6c019db8f59458878af7d166b56')
    )
