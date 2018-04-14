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
                /*
                byte[] c = originalEncode.GetBytes(str);
               byte[] d = Encoding.Convert(originalEncode, targetEncode, c);
                return targetEncode.GetString(d);
                */
                /*
                byte[] c = originalEncode.GetBytes(str);
                return targetEncode.GetString(c);
                */
                //str = Encoding.Convert(Encoding.UTF8, originalEncode, str);
                byte[] temp = originalEncode.GetBytes(str);
                byte[] targetEncodeBytes = Encoding.Convert(originalEncode, targetEncode, temp);



                char[] targetEncodeChars = new char[targetEncode.GetCharCount(targetEncodeBytes, 0, targetEncodeBytes.Length)];
                targetEncode.GetChars(targetEncodeBytes, 0, targetEncodeBytes.Length, targetEncodeChars, 0);
                string targetEncodeString = new string(targetEncodeChars);
                return targetEncodeString;


                //return targetEncode.GetString(targetEncodeBytes);
            }
            catch
            {
                Console.WriteLine("There is an exception.");
                return null;
            }
        }
    }
}
