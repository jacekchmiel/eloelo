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
import requests
import coloredlogs

DEFAULT_TESSERACT_CONFIG = "--oem 3 --psm 6"
DEFAULT_ELOELO_ADDR = "localhost:3000"
DEFAULT_SCREENSHOT_DIR = (
    "/mnt/c/Program Files (x86)/Steam/userdata/96608807/760/remote/570/screenshots"
)


class ProgramArgumentError(Exception):
    pass


def parse_program_arguments():
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(help="command")
    ocr_parser = subparsers.add_parser("ocr", help="run ocr on sigle file")
    ocr_parser.set_defaults(command="ocr")
    ocr_parser.add_argument(
        "target",
        type=Path,
        help="file to process",
        metavar="TARGET",
    )
    ocr_parser.add_argument(
        "--send-results", "-s", action="store_true", help="send results to eloelo"
    )
    ocr_parser.add_argument(
        "--eloelo_addr",
        type=str,
        help=f"eloelo server address. Default: {DEFAULT_ELOELO_ADDR}",
        default=DEFAULT_ELOELO_ADDR,
    )
    watch_parser = subparsers.add_parser(
        "watch", help="run ocr on every new file in dir"
    )
    watch_parser.set_defaults(command="watch")
    watch_parser.add_argument(
        "target",
        type=Path,
        help="directory to watch, can be specified as `default`",
        metavar="TARGET",
    )
    watch_parser.add_argument(
        "--retries",
        type=int,
        help="number of retries when processing image fails in watch mode (default: 1)",
        default=1,
    )
    watch_parser.add_argument(
        "--poll-period",
        type=float,
        help="directory watch poll period in seconds (default: 0.2)",
        default=0.2,
    )
    watch_parser.add_argument(
        "--eloelo_addr",
        type=str,
        help=f"eloelo server address. (default: {DEFAULT_ELOELO_ADDR})",
        default=DEFAULT_ELOELO_ADDR,
    )
    args = parser.parse_args()

    pprint.pprint(args)
    if args.command == "watch":
        if args.command == "watch" and str(args.target) == "default":
            args.target = Path(DEFAULT_SCREENSHOT_DIR)
            LOG.info("Using default dir to watch: %s", args.target)
        if not Path(args.target).resolve().is_dir():
            raise ProgramArgumentError("Watch target is not a dir")

    if args.command == "ocr":
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
coloredlogs.install(
    level="DEBUG", logger=LOG, fmt="%(asctime)s %(levelname)s %(message)s"
)


class PatheticDirectoryWatcher:
    def __init__(
        self, directory: Path, *, poll_period, retries
    ) -> "PatheticDirectoryWatcher":
        self._dir = directory
        self._seen_files = set()
        self._poll_period = poll_period
        self._failed_attempts = Counter()
        self._retries = retries

    def run(self, callback):
        self._seen = {f for f in self._dir.glob("*") if f.is_file()}
        LOG.info(f"Watching directory {self._dir}")

        while True:
            start = datetime.now()
            for f in self._dir.glob("*"):
                if not f.is_file() or not self._should_process(f):
                    continue
                self._run_callback(callback, f)
                self._seen.add(f)
            rest = self._poll_period - (datetime.now() - start).total_seconds()
            if rest > 0:
                time.sleep(rest)

    def _should_process(self, file: Path) -> bool:
        return file not in self._seen and self._failed_attempts[file] <= self._retries

    def _run_callback(self, callback, file: Path):
        try:
            callback(file)
        except KeyboardInterrupt:
            raise
        except Exception as e:
            if isinstance(e, requests.exceptions.ConnectTimeout):
                LOG.error(e)
            else:
                LOG.exception("Failed to process %s", file)
            self._failed_attempts[file] += 1


def send_results(addr, players):
    url = f"http://{addr}/api/v1/lobby_screenshot"
    response = requests.post(url, json={"playersInLobby": players}, timeout=3)
    if not response.ok:
        LOG.error("Server response: %s", response.content.decode("utf8"))
    response.raise_for_status()


def main():
    args = parse_program_arguments()
    logging.basicConfig(level=logging.DEBUG)

    if args.command == "watch":

        def handle_file(file: Path):
            result = read_any_screenshot_4k(file)
            LOG.info(pprint.pformat(result))
            if "lobby" not in result["type"]:
                return
            send_results(args.eloelo_addr, result["players"])

        watcher = PatheticDirectoryWatcher(
            args.target, poll_period=args.poll_period, retries=args.retries
        )
        watcher.run(handle_file)
    elif args.command == "ocr":
        result = read_any_screenshot_4k(args.target)
        LOG.info(pprint.pformat(result))
        if args.send:
            send_results(args.eloelo_addr, result["players"])


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        pass
    except ProgramArgumentError as e:
        print(e, file=sys.stderr)
        sys.exit(1)
