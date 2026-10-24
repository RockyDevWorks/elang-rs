phonemes {
  consonants {
    phoneme "t" {
      roman "t"
      when {
        next-phoneme "a"
        word-position "initial"
        group {
          previous-phoneme "s"
          syllable-position "coda"
        }
        or {
          previous-phoneme "z"
          next-phoneme "k"
        }
      }
    }
    phoneme "s" {
      roman "s"
    }
    phoneme "z" {
      roman "z"
    }
    phoneme "k" {
      roman "k"
    }
  }
  vowels {
    phoneme "a" {
      roman "a"
    }
  }
}
