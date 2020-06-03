import subprocess
import argparse
from multiprocessing import Pool
import time
import shutil
import os
import sys
import webbrowser


def runInstance(index):
    start = time.time()
    args = ["target/release/test_undo_redo", str(index)]
    subprocess.run(args, check=True, stdout=subprocess.DEVNULL)
    end = time.time()
    print("Process completed in " + str(end - start) + " seconds")


def run(num_actors):
    start = time.time()
    args = []
    for i in range(0, num_actors):
        args.append([i])
    with Pool(num_actors) as p:
        p.starmap(runInstance, args)
    end = time.time()
    print("Test completed in " + str(end - start) + " seconds")


if __name__ == '__main__':
    parser = argparse.ArgumentParser(
        description='Run undo/redo test')
    parser.add_argument('--num', type=int, default=1,
                        help="The number of actors to simulate")
    args = parser.parse_args()
    os.chdir("../")
    subprocess.run(["cargo", "build", "--release",
                    "-p", "test_undo_redo"], check=True)
    webbrowser.open_new_tab(
        "http://127.0.0.1/index.html?file=00000003-0003-0003-0003-000000000003")
    time.sleep(2)

    run(args.num)
