phonemes {
  consonants {
    phoneme "t" {
      roman "t"
      allophone "tʰ" {
        when {
          next-phoneme "a"
          word-position "initial"
        }
      }
    }
  }
  vowels {
    phoneme "a" {
      roman "a"
      allophone "ã" {
        when {
          previous-phoneme "t"
          word-position "final"
        }
      }
    }
  }
}
