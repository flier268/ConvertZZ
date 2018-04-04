using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace ConvertZZ
{
    public class TransferEncodeHelper
    {
        public static string TransferStr(string str, Encoding originalEncode, Encoding targetEncode)
        {
            try
            {
                byte[] unicodeBytes = originalEncode.GetBytes(str);
                byte[] asciiBytes = Encoding.Convert(originalEncode, targetEncode, unicodeBytes);
                char[] asciiChars = new char[targetEncode.GetCharCount(asciiBytes, 0, asciiBytes.Length)];
                targetEncode.GetChars(asciiBytes, 0, asciiBytes.Length, asciiChars, 0);
                string result = new string(asciiChars);
                return result;
            }
            catch
            {
                Console.WriteLine("There is an exception.");
                return "";
            }
        }
    }
}
