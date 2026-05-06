import sys

if __name__ == '__main__':
    # Unpack PSF2 fonts.

    file_name = sys.argv[1]
    output_name = 'font.txt'

    with open(file_name, 'rb') as file:
        data = file.read()
        print(data[0:32].hex())
        with open(output_name, 'w') as output_file:
            text_buffer = ''
            for byte in data[32:]:
                text_buffer += hex(byte) + ', '
            output_file.write(text_buffer)