import cv2
import pytesseract
import logging
import pprint
import argparse
from pathlib import Path
import sys
import re
from datetime import datetime
import time
from collections import Counter
import coloredlogs
import json

DEFAULT_TESSERACT_CONFIG = "--oem 3 --psm 6"
DEFAULT_SCREENSHOT_DIR = (
    "/mnt/c/Program Files (x86)/Steam/userdata/96608807/760/remote/570/screenshots"
)


class ProgramArgumentError(Exception):
    pass


def parse_program_arguments():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "target",
        type=Path,
        help="file to process",
        metavar="TARGET",
    )
    parser.add_argument(
        "--quiet", "-q", help="do not print unnecessary stuff", action="store_true"
    )

    args = parser.parse_args()

    if not Path(args.target).resolve().is_file():
        raise ProgramArgumentError("Target is not a file")

    return args


LOG = logging.getLogger()


def configure_logging(args):
    level = logging.DEBUG if not args.quiet else logging.WARNING
    coloredlogs.install(
        level=level, logger=LOG, fmt="%(asctime)s %(levelname)s %(message)s"
    )


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


class Reader:
    def __init__(self, coords):
        self.coords = coords

    def read_arcade_lobby(self, image):
        control_image = preprocess(crop(image, *self.coords.arcade_lobby_control_rect))
        control = image_to_string(control_image).strip().lower()

        players_image = preprocess(crop(image, *self.coords.arcade_lobby_players_rect))
        players = image_to_string(players_image)
        players = re.split("\s", players)
        players = [
            p.lower() for p in players if len(p) > 0 and f"{p[0]}{p[-1]}" != "[]"
        ]
        return {
            "type": "arcade_lobby" if "lobby" in control else "unknown",
            "control_arcade_lobby": control,
            "players": players,
        }

    def read_regular_lobby(self, image):
        control_image = preprocess(crop(image, *self.coords.regular_lobby_control_rect))
        control = image_to_string(control_image).strip().lower()

        radiant_image = preprocess(crop(image, *self.coords.regular_lobby_radiant_rect))
        dire_image = preprocess(crop(image, *self.coords.regular_lobby_dire_rect))
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


class Coords4k:
    def __init__(self):
        self.arcade_lobby_control_rect = ((3080, 130), (3250, 200))
        self.arcade_lobby_players_rect = ((3270, 500), (3800, 1500))

        self.regular_lobby_control_rect = ((3069, 1918), (3718, 1980))
        self.regular_lobby_radiant_rect = ((2544, 335), (2994, 830))
        self.regular_lobby_dire_rect = ((3215, 335), (3750, 830))


def scale(rect, factor):
    result = (
        (int(rect[0][0] * factor), int(rect[0][1] * factor)),
        (int(rect[1][0] * factor), int(rect[1][1] * factor)),
    )
    LOG.debug("Scaling %s by %s = %s", rect, factor, result)
    return result


class ScaledCoords:
    def __init__(self, coords, factor):
        self.arcade_lobby_control_rect = scale(coords.arcade_lobby_control_rect, factor)
        self.arcade_lobby_players_rect = scale(coords.arcade_lobby_players_rect, factor)

        self.regular_lobby_control_rect = scale(
            coords.regular_lobby_control_rect, factor
        )
        self.regular_lobby_radiant_rect = scale(
            coords.regular_lobby_radiant_rect, factor
        )
        self.regular_lobby_dire_rect = scale(coords.regular_lobby_dire_rect, factor)


def read_any_screenshot(filename) -> str:
    result = {}

    image = read(filename)
    res = image.shape[:2]
    LOG.info("image size: %s", res)

    if res == (2160, 3840):
        coords = Coords4k()
    elif res == (1440, 2560):
        coords = ScaledCoords(Coords4k(), 1440 / 2160)
    elif res == (1080, 1920):
        coords = ScaledCoords(Coords4k(), 0.5)
    else:
        raise RuntimeError("Display resolution %s not supported", res)

    reader = Reader(coords)

    # Need to check for arcade lobby first, since
    # control check for regular will also match arcade
    result.update(reader.read_arcade_lobby(image))
    if result["type"] == "arcade_lobby":
        LOG.info("Detected Dotka LP lobby screenshot")
        return result

    result.update(reader.read_regular_lobby(image))
    if result["type"] == "regular_lobby":
        LOG.info("Detected lobby screenshot")
        return result

    return result


def main():
    args = parse_program_arguments()
    configure_logging(args)

    result = read_any_screenshot(args.target)
    print(json.dumps(result))


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        pass
    except ProgramArgumentError as e:
        print(e, file=sys.stderr)
        sys.exit(1)
