import logging
import argparse
from pathlib import Path
import sys
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
    parser = argparse.ArgumentParser(
        description="Watches screenshot directory and sends screenshots to eloelo to process."
    )
    parser.add_argument(
        "--retries",
        type=int,
        help="number of retries when processing image fails in watch mode (default: 1)",
        default=1,
    )
    parser.add_argument(
        "--poll-period",
        type=float,
        help="directory watch poll period in seconds (default: 0.2)",
        default=0.2,
    )
    parser.add_argument(
        "--eloelo-addr",
        type=str,
        help=f"eloelo server address. (default: {DEFAULT_ELOELO_ADDR})",
        default=DEFAULT_ELOELO_ADDR,
    )
    parser.add_argument(
        "target",
        type=Path,
        help="directory to watch, can be specified as `default`",
        metavar="TARGET",
    )
    parser.add_argument("--request-timeout", type=float, metavar="SECONDS", default=3.0)
    parser.add_argument(
        "--quiet", "-q", help="do not print unnecessary stuff", action="store_true"
    )
    args = parser.parse_args()

    args.target = Path(DEFAULT_SCREENSHOT_DIR)
    LOG.info("Using default dir to watch: %s", args.target)
    if not Path(args.target).resolve().is_dir():
        raise ProgramArgumentError("Watch target is not a dir")

    return args


LOG = logging.getLogger()


def configure_logging(args):
    level = logging.DEBUG if not args.quiet else logging.WARNING
    coloredlogs.install(
        level=level, logger=LOG, fmt="%(asctime)s %(levelname)s %(message)s"
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


def send_raw_file(addr, path, *, timeout):
    url = f"http://{addr}/api/v1/dota_screenshot"
    path = Path(path)
    ext = path.suffix[1:]  # skip dot
    if ext in ("png", "jpg", "bmp"):
        content_type = f"image/{ext}"
    else:
        content_type = "application/octet-stream"
    data = path.read_bytes()
    LOG.debug("POST %s, %s bytes -> %s", path, len(data), url)
    response = requests.post(
        url,
        data=data,
        headers={"Content-Type": content_type},
        timeout=timeout,
    )
    if not response.ok:
        LOG.error("Server response: %s", response.content.decode("utf8"))
    response.raise_for_status()


def main():
    args = parse_program_arguments()
    configure_logging(args)

    LOG.info("EloElo address: %s", args.eloelo_addr)
    LOG.info("Request timeout: %s", args.request_timeout)

    def handle_file(file: Path):
        LOG.info(f"Sending ${file}")
        send_raw_file(args.eloelo_addr, file, timeout=args.request_timeout)

    watcher = PatheticDirectoryWatcher(
        args.target, poll_period=args.poll_period, retries=args.retries
    )
    watcher.run(handle_file)


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        pass
    except ProgramArgumentError as e:
        print(e, file=sys.stderr)
        sys.exit(1)
