import cv2
import pytesseract
import logging
import pprint
import argparse
from pathlib import Path
import sys
import re

DEFAULT_TESSERACT_CONFIG = "--oem 3 --psm 6"
DEFAULT_ELOELO_ADDR = "localhost:3001"


class ProgramArgumentError(Exception):
    pass


def parse_program_arguments():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "target",
        type=Path,
        help="file to process or directory to watch",
        metavar="TARGET",
    )
    parser.add_argument(
        "--watch",
        action="store_true",
        help="process screenshots as they appear in TARGET dir",
    )
    parser.add_argument(
        "--eloelo_addr",
        type=str,
        help=f"eloelo server address. Default: {DEFAULT_ELOELO_ADDR}",
        default=DEFAULT_ELOELO_ADDR,
    )
    args = parser.parse_args()

    if args.watch and not Path(args.target).resolve().is_dir():
        raise ProgramArgumentError("Watch target is not a dir")

    if not Path(args.target).resolve().is_file():
        raise ProgramArgumentError("Target is not a file")

    return args


def image_to_string(image, config=DEFAULT_TESSERACT_CONFIG) -> str:
    return pytesseract.image_to_string(image, config=config)


def get_grayscale(image):
    return cv2.cvtColor(image, cv2.COLOR_BGR2GRAY)


def invert(image):
    return 255 - image


def thresholding(image):
    return cv2.threshold(image, 0, 255, cv2.THRESH_BINARY + cv2.THRESH_OTSU)[1]


def crop(image, top_left, bottom_right):
    x0, y0 = top_left
    x1, y1 = bottom_right
    return image[y0:y1, x0:x1]


def read(filename):
    return cv2.imread(filename)


def preprocess(image):
    image = get_grayscale(image)
    image = invert(image)
    image = thresholding(image)
    return image


def is_match_results_screenshot_4k(image):
    control_image = crop(image, (460, 400), (1200, 530))
    control_image_text = image_to_string(control_image)
    return "dotka" in control_image_text.lower()


def read_match_results_screenshot_4k(image):
    # mask hero icons
    cv2.rectangle(image, (1370, 565), (1450, 1110), (255, 255, 255), -1)
    cv2.rectangle(image, (2400, 565), (2480, 1110), (255, 255, 255), -1)

    # crop to result table
    results_image = crop(image, (900, 500), (3000, 1200))
    duration_image = crop(image, (3170, 400), (3400, 550))
    match_id_image = crop(image, (470, 1750), (2500, 1850))

    tables = image_to_string(results_image)
    duration = image_to_string(duration_image)
    match_id = image_to_string(match_id_image)

    return {
        "tables": tables,
        "duration": duration,
        "match_id": match_id,
    }


def read_arcade_lobby_4k(filename):
    image = read(filename)
    # cv2_imshow(image)

    control_image = preprocess(crop(image, (3080, 130), (3250, 200)))
    # cv2_imshow(control_image)

    control = image_to_string(control_image).strip().lower()

    players_image = preprocess(crop(image, (3270, 500), (3800, 1500)))
    # cv2_imshow(players_image)

    players = image_to_string(players_image)
    players = re.split("\s", players)
    players = [p.lower() for p in players if len(p) > 0 and f"{p[0]}{p[-1]}" != "[]"]
    return {
        "type": "arcade_lobby" if "lobby" in control else "unknown",
        "control_arcade_lobby": control,
        "players": players,
    }


def read_regular_lobby_4k(filename):
    image = read(filename)
    # cv2_imshow(image)

    control_image = preprocess(crop(image, (3069, 1918), (3718, 1980)))
    # cv2_imshow(control_image)

    control = image_to_string(control_image).strip().lower()

    radiant_image = preprocess(crop(image, (2544, 335), (2994, 830)))
    # cv2_imshow(radiant_image)

    # image = read('lobby3.jpg')
    dire_image = preprocess(crop(image, (3215, 335), (3750, 830)))
    # cv2_imshow(dire_image)

    players = image_to_string(radiant_image) + image_to_string(dire_image)
    players = [
        p.lower()
        for p in re.split("\s", players)
        if len(p) > 0 and f"{p[0]}{p[-1]}" != "[]"
    ]

    return {
        "type": "regular_lobby" if "lobby" in control else "unknown",
        "control_regular_lobby": control,
        "players": players,
    }


def read_any_screenshot_4k(filename) -> str:
    result = {}
    # Need to check for arcade lobby first, since
    # control check for regular will also match arcade
    result.update(read_arcade_lobby_4k(filename))
    if result["type"] == "arcade_lobby":
        LOG.info("Detected Dotka LP lobby screenshot")
        return result

    result.update(read_regular_lobby_4k(filename))
    if result["type"] == "regular_lobby":
        LOG.info("Detected lobby screenshot")
        return result

    return result


LOG = logging.getLogger()


def main():
    args = parse_program_arguments()
    logging.basicConfig(level=logging.DEBUG)

    if args.watch:
        LOG.info(f"Watching directory {args.target}")
        LOG.error("NOT IMPLEMENTED YET")
        sys.exit(1)

    result = read_any_screenshot_4k(args.target)

    pprint.pprint(result)


if __name__ == "__main__":
    try:
        main()
    except ProgramArgumentError as e:
        print(e, file=sys.stderr)
        sys.exit(1)
